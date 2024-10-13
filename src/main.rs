//! An HID/Keyboard device that types preloaded text.

#![no_std]
#![no_main]

use bsp::entry;
use panic_halt as _;

// Device specific
use pimoroni_tiny2040 as bsp;

use bsp::hal;
use bsp::hal::{clocks::Clock, gpio, pac, pac::interrupt};

// Locking
use core::cell::RefCell;
use critical_section::Mutex;

// USB Device support
use usb_device::{class_prelude::*, prelude::*};

// USB Human Interface Device (HID) Class support
use usbd_hid::descriptor::generator_prelude::*;
use usbd_hid::descriptor::KeyboardReport;
use usbd_hid::hid_class::HIDClass;

mod leds;
mod text;

/// The USB Device Driver (shared with the interrupt).
static mut USB_DEVICE: Option<UsbDevice<hal::usb::UsbBus>> = None;

/// The USB Bus Driver (shared with the interrupt).
static mut USB_BUS: Option<UsbBusAllocator<hal::usb::UsbBus>> = None;

/// The USB Human Interface Device Driver (shared with the interrupt).
static mut USB_HID: Option<HIDClass<hal::usb::UsbBus>> = None;

/// The pin used as an input button
static CLICK_BUTTON: Mutex<RefCell<Option<ClickButtonPin>>> = Mutex::new(RefCell::new(None));

/// The global state
/// FIXME: the interrupt triggers once at the start (it seems) so we flip the initial value
static G_STATE: Mutex<RefCell<State>> = Mutex::new(RefCell::new(true));

type State = bool; // true: show green and type; false: show red and don't type

type ClickButtonPin =
    gpio::Pin<gpio::bank0::Gpio23, gpio::FunctionSio<gpio::SioInput>, gpio::PullUp>;

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

    let mut led_channels = leds::init_pwm(
        pac.PWM,
        &mut pac.RESETS,
        (pins.led_red, pins.led_green, pins.led_blue),
    );

    // Our button input
    let button_pin: ClickButtonPin = pins.bootsel.into_pull_up_input();

    // Trigger on the 'falling edge' of the input pin.
    // This will happen as the button is being pressed
    button_pin.set_interrupt_enabled(gpio::Interrupt::EdgeLow, true);

    critical_section::with(|cs| {
        CLICK_BUTTON.borrow(cs).replace(Some(button_pin));
    });

    // Set up the USB driver
    let usb_bus = UsbBusAllocator::new(hal::usb::UsbBus::new(
        pac.USBCTRL_REGS,
        pac.USBCTRL_DPRAM,
        clocks.usb_clock,
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

    let core = pac::CorePeripherals::take().unwrap();
    let mut delay = cortex_m::delay::Delay::new(core.SYST, clocks.system_clock.freq().to_Hz());

    let mut n_written: usize = 0;

    // Main loop
    loop {
        // Check the state
        let on: bool = critical_section::with(|cs| {
            let f = G_STATE.borrow(cs).borrow();
            f.clone()
        });

        // Show state
        if on {
            led_channels.green();
        } else {
            led_channels.red();
        }

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

        delay.delay_ms(50);
        push_hid_report(rep_up).ok().unwrap_or(0);
        delay.delay_ms(100);
    }
}

/// USB

/// Submit a new mouse movement report to the USB stack.
///
/// We do this with interrupts disabled, to avoid a race hazard with the USB IRQ.
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
    // The `#[interrupt]` attribute covertly converts this to `&'static mut Option<LedAndButton>`
    static mut BUTTON: Option<ClickButtonPin> = None;

    // This is one-time lazy initialisation. We steal the variables given to us
    // via `GLOBAL_PINS`.
    if BUTTON.is_none() {
        critical_section::with(|cs| {
            *BUTTON = CLICK_BUTTON.borrow(cs).take();
        });
    }

    if let Some(button) = BUTTON {
        if button.interrupt_status(gpio::Interrupt::EdgeLow) {
            button.clear_interrupt(gpio::Interrupt::EdgeLow);
        }
    }

    critical_section::with(|cs| {
        let old: bool = G_STATE.borrow(cs).borrow().clone();
        G_STATE.borrow(cs).replace(!old);
    });
}
