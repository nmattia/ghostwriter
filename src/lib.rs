//! Main ghostwriter library for async & input handling

#![no_std]

use panic_halt as _;

// Device specific
use pimoroni_tiny2040 as bsp;

use bsp::hal;
use bsp::hal::{gpio, pac::interrupt};

// Locking
use core::cell::RefCell;
use critical_section::Mutex;

use core::clone::Clone;
use core::option::Option;
use core::option::Option::{None, Some};
use core::pin::Pin;
use futures::task::{Context, Poll};
use futures::Future;

use bsp::hal::pac;

pub mod leds;
pub mod usb;

/// The pin used as an input button. Global value set once by the setup, and then read
/// once (stolen, taken) from the button interrupt handler.
type ButtonPin = gpio::Pin<gpio::bank0::Gpio23, gpio::FunctionSio<gpio::SioInput>, gpio::PullUp>;
static _BUTTON_PIN: Mutex<RefCell<Option<ButtonPin>>> = Mutex::new(RefCell::new(None));

/// Global state shared between the eventloop and the button interrupt handler.
static BUTTON_DOWN: Mutex<RefCell<bool>> = Mutex::new(RefCell::new(false));

/// Sets up the ghostwriter silicon. Booooh!
pub fn setup() -> (hal::Timer, leds::LEDChannels) {
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
    let button_pin = pins.bootsel.into_pull_up_input();

    setup_button(button_pin);

    // Prepare timer
    let timer = hal::Timer::new(pac.TIMER, &mut pac.RESETS, &clocks);

    // Set up the USB driver
    let usb_clock = clocks.usb_clock;
    usb::setup_usb_driver(
        pac.USBCTRL_REGS,
        pac.USBCTRL_DPRAM,
        usb_clock,
        &mut pac.RESETS,
    );

    unsafe {
        // Enable the USB interrupt
        pac::NVIC::unmask(hal::pac::Interrupt::USBCTRL_IRQ);

        // Enable the button click interrupt
        pac::NVIC::unmask(pac::Interrupt::IO_IRQ_BANK0);
    };

    (timer, led_channels)
}

// Duration
pub type Duration = hal::fugit::Duration<u64, 1, 1000000>;

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
pub struct Scheduler<'a> {
    timer: &'a hal::Timer,
}

impl<'a> Scheduler<'a> {
    pub fn new(timer: &'a hal::Timer) -> Self {
        Scheduler { timer: &timer }
    }

    pub fn sleep_ms(self: &Self, v: u64) -> Sleep<'a> {
        let now: Duration = self.timer.get_counter().duration_since_epoch();
        let delta: Duration = Duration::millis(v);
        Sleep {
            target: now + delta,
            timer: &self.timer,
        }
    }

    pub fn wait_for_press(self: &Self) -> Input {
        Input {}
    }

    pub fn now(self: &Self) -> Duration {
        self.timer.get_counter().duration_since_epoch()
    }
}

/// BUTTON

pub fn setup_button(button_pin: ButtonPin) {
    // Trigger on the 'falling edge' of the input pin.
    // This will happen as the button is being pressed
    button_pin.set_interrupt_enabled(gpio::Interrupt::EdgeLow, true);

    critical_section::with(|cs| {
        _BUTTON_PIN.borrow(cs).replace(Some(button_pin));
        BUTTON_DOWN.borrow(cs).replace(false);
    });
}

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
