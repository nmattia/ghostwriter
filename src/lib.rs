//! Main ghostwriter library for async & input handling

#![no_std]

use panic_halt as _;

// Device specific
use pimoroni_tiny2040 as bsp;

use bsp::hal;
use bsp::hal::rosc::{Enabled, RingOscillator};

// Locking

use bsp::hal::pac;

pub mod input;
pub mod leds;
pub mod scheduler;
pub mod sleep;
pub mod usb;

pub use scheduler::{Duration, Scheduler};

use input::Keypress;
use scheduler::Instant;

/// Sets up the ghostwriter silicon. Booooh!
///
/// NOTE: The timer's `alarm0` has already been taken, do _not_ try to use it.
pub fn setup() -> (hal::Timer, leds::LEDChannels, RingOscillator<Enabled>) {
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
    input::setup_button(button_pin);

    // Prepare timer & alarm
    let mut timer = hal::Timer::new(pac.TIMER, &mut pac.RESETS, &clocks);
    let alarm0 = timer.alarm_0().unwrap();
    sleep::setup_sleep(alarm0);

    // Set up the USB driver
    let usb_clock = clocks.usb_clock;
    usb::setup_usb_driver(
        pac.USBCTRL_REGS,
        pac.USBCTRL_DPRAM,
        usb_clock,
        &mut pac.RESETS,
    );

    // Ring oscillator, used as a RNG
    let rosc = RingOscillator::new(pac.ROSC).initialize();

    unsafe {
        // Enable the USB interrupt
        pac::NVIC::unmask(hal::pac::Interrupt::USBCTRL_IRQ);

        // Enable the button click interrupt
        pac::NVIC::unmask(pac::Interrupt::IO_IRQ_BANK0);

        // Enable the timer interrupt
        pac::NVIC::unmask(pac::Interrupt::TIMER_IRQ_0);
    };

    (timer, led_channels, rosc)
}

/// Wait until an event is available to process.
///
/// In practice this registers a wake up alarm if necessary and puts the cortex to
/// sleep by calling Wait For Interrupt (and not actually the cortex' Wait For Event, but "event"
/// make more sense in the context of an event loop).
pub fn wait_for_event() {
    critical_section::with(|cs| {
        sleep::wind_alarm(cs);

        // Since both sleep & input wait for an interrupt, we can just put the core to sleep
        // and wait for an interrupt.
        // We put this _inside_ the critical_section, otherwise the interrupts will have been
        // re-enabled before we reach WFI and the alarm might have triggered before.
        //
        // It's fine to WFI inside critical section; even though interrupts are "disabled" the chip
        // will still be woken up.
        cortex_m::asm::wfi();
    });
}

#[macro_export]
/// A macro that runs and polls ghostwriter futures in a loop
/// (the future executor)
macro_rules! run {
    ( $( $x:ident ),* ) => {
        {
        let waker = ::futures::task::noop_waker();
        let mut ctx = ::futures::task::Context::from_waker(&waker);

        $(
        let mut $x = pin!($x);
        )*

        loop {
            $(
            let _: Poll<()> = $x.as_mut().poll(&mut ctx);
            )*
            ::ghostwriter::wait_for_event();
        }
        }
    };
}
