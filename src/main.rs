//! An HID/Keyboard device that types preloaded text.

#![no_std]
#![no_main]

use bsp::entry;
use panic_halt as _;

// Device specific
use pimoroni_tiny2040 as bsp;

use bsp::hal;
use bsp::hal::{gpio, pac, pac::interrupt};

// Locking
use core::cell::RefCell;
use critical_section::Mutex;

// USB Device support
use usb_device::{class_prelude::*, prelude::*};

// USB Human Interface Device (HID) Class support
use usbd_hid::descriptor::generator_prelude::*;
use usbd_hid::descriptor::KeyboardReport;
use usbd_hid::hid_class::HIDClass;

use core::pin::{pin, Pin};
use futures::task::{noop_waker, Context, Poll};
use futures::Future;

use libm;

mod leds;
mod text;

/// The USB Device Driver (shared with the interrupt).
static mut USB_DEVICE: Option<UsbDevice<hal::usb::UsbBus>> = None;

/// The USB Bus Driver (shared with the interrupt).
static mut USB_BUS: Option<UsbBusAllocator<hal::usb::UsbBus>> = None;

/// The USB Human Interface Device Driver (shared with the interrupt).
static mut USB_HID: Option<HIDClass<hal::usb::UsbBus>> = None;

/// The pin used as an input button. Global value set once by the setup, and then read
/// once (stolen, taken) from the button interrupt handler.
static _BUTTON_PIN: Mutex<RefCell<Option<ButtonPin>>> = Mutex::new(RefCell::new(None));

/// Global state shared between the eventloop and the button interrupt handler.
static BUTTON_DOWN: Mutex<RefCell<State>> = Mutex::new(RefCell::new(false));

type State = bool; // true: currently running, false: not running

type ButtonPin = gpio::Pin<gpio::bank0::Gpio23, gpio::FunctionSio<gpio::SioInput>, gpio::PullUp>;

/// Entry point to our bare-metal application.
///
/// The `#[entry]` macro ensures the Cortex-M start-up code calls this function
/// as soon as all global variables are initialised.
#[entry]
fn main() -> ! {
    // Grab our singleton objects
    let mut pac = pac::Peripherals::take().unwrap();

    // Set up the watchdog driver - needed by the clock setup code
    let mut watchdog = hal::Watchdog::new(pac.WATCHDOG);

    // Configure the clocks
    //
    // The default is to generate a 125 MHz system clock
    let clocks = hal::clocks::init_clocks_and_plls(
        bsp::XOSC_CRYSTAL_FREQ,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    let sio = hal::Sio::new(pac.SIO);
    let pins = bsp::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    let led_channels = leds::init_pwm(
        pac.PWM,
        &mut pac.RESETS,
        (pins.led_red, pins.led_green, pins.led_blue),
    );

    // Our button input
    let button_pin: ButtonPin = pins.bootsel.into_pull_up_input();

    // Trigger on the 'falling edge' of the input pin.
    // This will happen as the button is being pressed
    button_pin.set_interrupt_enabled(gpio::Interrupt::EdgeLow, true);

    critical_section::with(|cs| {
        _BUTTON_PIN.borrow(cs).replace(Some(button_pin));
        BUTTON_DOWN.borrow(cs).replace(false);
    });

    // Prepare timer
    let timer = hal::Timer::new(pac.TIMER, &mut pac.RESETS, &clocks);

    // Set up the USB driver
    let usb_clock = clocks.usb_clock;
    let usb_bus = UsbBusAllocator::new(hal::usb::UsbBus::new(
        pac.USBCTRL_REGS,
        pac.USBCTRL_DPRAM,
        usb_clock,
        true,
        &mut pac.RESETS,
    ));

    unsafe {
        // Note (safety): This is safe as interrupts haven't been started yet
        USB_BUS = Some(usb_bus);
    }

    let usb_dev = {
        // Grab a reference to the USB Bus allocator. We are promising to the
        // compiler not to take mutable access to this global variable whilst this
        // reference exists!
        let bus_ref = unsafe { USB_BUS.as_ref().unwrap() };

        // Set up the USB HID Class Device driver, providing Mouse Reports
        let usb_hid = HIDClass::new(bus_ref, KeyboardReport::desc(), 60);
        unsafe {
            // Note (safety): This is safe as interrupts haven't been started yet.
            USB_HID = Some(usb_hid);
        }

        // Create a USB device with a fake VID and PID
        UsbDeviceBuilder::new(bus_ref, UsbVidPid(0x16c0, 0x27da))
            .strings(&[StringDescriptors::default()
                .manufacturer("Noe's ghostwrtier")
                .product("Up-down-up-down")
                .serial_number("NOPE")])
            .unwrap()
            .device_class(0)
            .build()
    };
    unsafe {
        // Note (safety): This is safe as interrupts haven't been started yet
        USB_DEVICE = Some(usb_dev);
    }

    unsafe {
        // Enable the USB interrupt
        pac::NVIC::unmask(hal::pac::Interrupt::USBCTRL_IRQ);

        // Enable the button click interrupt
        pac::NVIC::unmask(pac::Interrupt::IO_IRQ_BANK0);
    };

    run(timer, led_channels)
}

// Duration
type Duration = hal::fugit::Duration<u64, 1, 1000000>;

// Async/await sleep
pub struct Sleep<'a> {
    target: Duration,
    timer: &'a hal::Timer,
}

impl Future for Sleep<'_> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _ctx: &mut Context<'_>) -> Poll<Self::Output> {
        let now: Duration = self.timer.get_counter().duration_since_epoch();

        if self.target < now {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

/// Async/await waiting for user input (button press)
pub struct Input {}

impl Future for Input {
    type Output = bool;

    fn poll(self: Pin<&mut Self>, _ctx: &mut Context<'_>) -> Poll<Self::Output> {
        let was_pressed = critical_section::with(|cs| {
            // Check if button has been pressed
            let was_pressed: bool = BUTTON_DOWN.borrow(cs).borrow().clone();
            BUTTON_DOWN.borrow(cs).replace(false);
            was_pressed
        });

        // If the button was pressed, we're ready to continue
        if was_pressed {
            Poll::Ready(true)
        } else {
            Poll::Pending
        }
    }
}

// Wrapper around timer so that poll knows the time
struct Scheduler<'a> {
    timer: &'a hal::Timer,
}

impl<'a> Scheduler<'a> {
    fn sleep_ms(self: &Self, v: u64) -> Sleep<'a> {
        let now: Duration = self.timer.get_counter().duration_since_epoch();
        let delta: Duration = Duration::millis(v);
        Sleep {
            target: now + delta,
            timer: &self.timer,
        }
    }

    fn wait_for_press(self: &Self) -> Input {
        Input {}
    }

    fn now(self: &Self) -> Duration {
        self.timer.get_counter().duration_since_epoch()
    }
}

// Future executor that loops through all futures and polls them consistently.
pub fn run(timer: hal::Timer, led_channels: leds::LEDChannels) -> ! {
    let scheduler = Scheduler { timer: &timer };

    // The global state
    // NOTE: the interrupt triggers once at the start (it seems) but because the input handler
    // has some debouncing (computed from epoch) this doesn't matter and the "press" is not
    // registered.
    let state: Mutex<RefCell<State>> = Mutex::new(RefCell::new(false));

    let handle_leds = handle_leds(&scheduler, &state, led_channels);
    let mut handle_leds = pin!(handle_leds);

    let handle_usb = handle_usb(&scheduler, &state);
    let mut handle_usb = pin!(handle_usb);

    let handle_input = handle_input(&scheduler, &state);
    let mut handle_input = pin!(handle_input);

    let waker = noop_waker();
    let mut ctx = Context::from_waker(&waker);

    loop {
        let _: Poll<()> = handle_leds.as_mut().poll(&mut ctx);
        let _: Poll<()> = handle_usb.as_mut().poll(&mut ctx);
        let _: Poll<()> = handle_input.as_mut().poll(&mut ctx);
    }
}

async fn handle_input<'a>(scheduler: &Scheduler<'a>, state: &Mutex<RefCell<State>>) {
    // Memory of the last (registered, acknowledged) press, needed for debouncing
    let mut last_press: Duration = Duration::secs(0);
    loop {
        scheduler.wait_for_press().await;

        // If the last press occurred in the last few moments (and technically within
        // the first few moments after boot), pretend this current press didn't happen.
        let now = scheduler.now();
        if now.to_millis() - last_press.to_millis() < 300 {
            continue;
        }
        last_press = now;

        // Button was pressed, so flip the state.
        critical_section::with(|cs| {
            let old: bool = state.borrow(cs).borrow().clone();
            state.borrow(cs).replace(!old);
        });
    }
}

/// Animate the LEDs based on the state
async fn handle_leds<'a>(
    scheduler: &Scheduler<'a>,
    state: &Mutex<RefCell<State>>,
    mut led_channels: leds::LEDChannels,
) {
    let mut set = |elapsed_millis: f64| {
        const TAU: f64 = 2.0 * core::f64::consts::PI;

        let on = critical_section::with(|cs| state.borrow(cs).borrow().clone());

        // Period of a blink
        let period_millis = if on { 1000.0 } else { 2000.0 };

        // How far we are in one blink
        let x = elapsed_millis / period_millis;

        // Sine waveform, shifted to [+1, -1]
        let x = 0.5 * libm::sin(x * TAU) + 0.5;

        led_channels.set_rgb(x / 3.0, x / 10.0, x);
    };

    // Approximate elapsed time
    let mut elapsed_millis: f64 = 0.0;

    const DELAY_MILLIS: u64 = 50;

    loop {
        set(elapsed_millis);
        scheduler.sleep_ms(DELAY_MILLIS).await;
        elapsed_millis += DELAY_MILLIS as f64;
    }
}

async fn handle_usb<'a>(scheduler: &Scheduler<'a>, state: &Mutex<RefCell<State>>) {
    let mut n_written: usize = 0;
    loop {
        let on: bool = critical_section::with(|cs| {
            let f = state.borrow(cs).borrow();
            f.clone()
        });

        // Apply state (USB stuff)
        let c = if !on {
            0x00
        } else {
            let chr = text::TEXT.as_bytes()[n_written];
            let keycode = text::char_to_keycode(chr);
            n_written = (n_written + 1) % text::TEXT.len();
            keycode
        };

        let rep_down = KeyboardReport {
            modifier: 0,
            reserved: 0,
            leds: 0,
            keycodes: [c, 0, 0, 0, 0, 0],
        };
        let rep_up = KeyboardReport {
            modifier: 0,
            reserved: 0,
            leds: 0,
            keycodes: [0x00, 0, 0, 0, 0, 0],
        };

        push_hid_report(rep_down).ok().unwrap_or(0);

        scheduler.sleep_ms(50).await;
        push_hid_report(rep_up).ok().unwrap_or(0);
        scheduler.sleep_ms(100).await;
    }
}

/// USB

/// Submit a new HID report to the USB stack.
///
/// We do this with interrupts disabled (critical_section), to avoid a race hazard
/// with the USB IRQ.
fn push_hid_report(report: KeyboardReport) -> Result<usize, usb_device::UsbError> {
    critical_section::with(|_| unsafe {
        // Now interrupts are disabled, grab the global variable and, if
        // available, send it a HID report
        USB_HID.as_mut().map(|hid| hid.push_input(&report))
    })
    .unwrap()
}

/// This function is called whenever the USB Hardware generates an Interrupt
/// Request.
#[interrupt]
unsafe fn USBCTRL_IRQ() {
    // Handle USB request
    let usb_dev = USB_DEVICE.as_mut().unwrap();
    let usb_hid = USB_HID.as_mut().unwrap();
    usb_dev.poll(&mut [usb_hid]);
}

/// BUTTON

#[interrupt]
fn IO_IRQ_BANK0() {
    // The `#[interrupt]` attribute covertly converts this to `&'static mut Option<...>`
    static mut BUTTON_PIN: Option<ButtonPin> = None;

    // This is one-time lazy initialisation. We steal the global interrupt variables.
    if BUTTON_PIN.is_none() {
        critical_section::with(|cs| {
            *BUTTON_PIN = _BUTTON_PIN.borrow(cs).take();
        });
    }

    if let Some(button) = BUTTON_PIN {
        if button.interrupt_status(gpio::Interrupt::EdgeLow) {
            button.clear_interrupt(gpio::Interrupt::EdgeLow);
            critical_section::with(|cs| {
                BUTTON_DOWN.borrow(cs).replace(true);
            });
        }
    }
}
