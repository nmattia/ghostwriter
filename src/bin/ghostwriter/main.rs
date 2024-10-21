//! An HID/Keyboard device that types preloaded text.

#![no_std]
#![no_main]

use bsp::entry;

// Device specific
use pimoroni_tiny2040 as bsp;

use bsp::hal;

// Locking
use core::cell::RefCell;
use critical_section::Mutex;

// USB Human Interface Device (HID) Class support
use usbd_hid::descriptor::KeyboardReport;

use core::pin::pin;
use futures::task::{noop_waker, Context, Poll};
use futures::Future;

use libm;

use ghostwriter::leds;

mod text;

type State = bool; // true: currently running, false: not running

/// Entry point to our bare-metal application.
///
/// The `#[entry]` macro ensures the Cortex-M start-up code calls this function
/// as soon as all global variables are initialised.
#[entry]
fn main() -> ! {
    let (timer, led_channels) = ghostwriter::setup();

    run(timer, led_channels)
}

// Future executor that loops through all futures and polls them consistently.
pub fn run(timer: hal::Timer, led_channels: crate::leds::LEDChannels) -> ! {
    let scheduler = ghostwriter::Scheduler::new(&timer);

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

async fn handle_input<'a>(scheduler: &ghostwriter::Scheduler<'a>, state: &Mutex<RefCell<State>>) {
    // Memory of the last (registered, acknowledged) press, needed for debouncing
    let mut last_press: ghostwriter::Duration = ghostwriter::Duration::secs(0);
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
    scheduler: &ghostwriter::Scheduler<'a>,
    state: &Mutex<RefCell<State>>,
    mut led_channels: crate::leds::LEDChannels,
) {
    let mut set = |elapsed_millis: f64| {
        const TAU: f64 = 2.0 * core::f64::consts::PI;

        let on = critical_section::with(|cs| state.borrow(cs).borrow().clone());

        // Period of a blink
        let period_millis = if on { 1000.0 } else { 3000.0 };

        // How far we are in one blink
        let x = elapsed_millis / period_millis;

        // Sine waveform, shifted to [+1, -1]
        let x = 0.5 * libm::sin(x * TAU) + 0.5;

        if on {
            led_channels.set_rgb(x / 1.0, x / 9.0, x);
        } else {
            led_channels.set_rgb(x / 3.0, x / 5.0, x / 4.0);
        }
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

async fn handle_usb<'a>(scheduler: &ghostwriter::Scheduler<'a>, state: &Mutex<RefCell<State>>) {
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

        ghostwriter::usb::push_hid_report(rep_down)
            .ok()
            .unwrap_or(0);

        scheduler.sleep_ms(50).await;
        ghostwriter::usb::push_hid_report(rep_up).ok().unwrap_or(0);
        scheduler.sleep_ms(100).await;
    }
}
