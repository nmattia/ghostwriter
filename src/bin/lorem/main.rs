//! An HID/Keyboard device that types preloaded text.

#![no_std]
#![no_main]

use bsp::entry;

use defmt::debug;
use defmt_rtt as _;

// Device specific
use pimoroni_tiny2040 as bsp;

use bsp::hal;
use bsp::hal::rosc::{Enabled, RingOscillator};

// Locking
use core::cell::RefCell;
use critical_section::Mutex;

// USB Human Interface Device (HID) Class support
use usbd_hid::descriptor::KeyboardReport;

use core::f64::consts::TAU;
use core::pin::pin;
use futures::select_biased;
use futures::task::Poll;
use futures::Future;
use futures::FutureExt;

use rand_distr::{ChiSquared, Distribution, Normal};

use ghostwriter::leds;
use ghostwriter::Duration;

mod text;

// Whether we're typing or not
#[derive(Clone, PartialEq, Eq)]
enum State {
    Typing,
    Stopped,
}

/// Entry point to our bare-metal application.
///
/// The `#[entry]` macro ensures the Cortex-M start-up code calls this function
/// as soon as all global variables are initialised.
#[entry]
fn main() -> ! {
    debug!("ghostwriter booting");
    let (timer, led_channels, rosc) = ghostwriter::setup();

    run(timer, led_channels, rosc)
}

// Run the lorem program
pub fn run(
    timer: hal::Timer,
    led_channels: ghostwriter::leds::LEDChannels,
    mut rosc: RingOscillator<Enabled>,
) -> ! {
    let scheduler = ghostwriter::Scheduler::new(&timer);

    // The global state (communication between LEDs & main loop)
    let state: Mutex<RefCell<State>> = Mutex::new(RefCell::new(State::Stopped));

    let handle_leds = handle_leds(&scheduler, &state, led_channels);
    let handle_usb = handle_usb(&scheduler, &state, &mut rosc);

    ghostwriter::run!(handle_leds, handle_usb)
}

/// Animate the LEDs based on the state
async fn handle_leds<'a>(
    scheduler: &ghostwriter::Scheduler<'a>,
    state: &Mutex<RefCell<State>>,
    mut led_channels: crate::leds::LEDChannels,
) {
    let read_state = || critical_section::with(|cs| state.borrow(cs).borrow().clone());

    #[allow(clippy::eq_op)]
    let state_color = |state: &State| match state {
        State::Typing => (1.0 / 1.0, 1.0 / 9.0, 1.0 / 1.0),
        State::Stopped => (1.0 / 3.0, 1.0 / 5.0, 1.0 / 4.0),
    };

    type Color = (f64, f64, f64);
    struct ColorAnimation {
        started_at: Duration, /* timestamp at which animation was started */
        start_color: Color,
        diff_color: Color,
    }

    // Total animation duration
    const ANIM_DURATION: Duration = Duration::millis(300);

    // Animation delay (pause between LED adjustments)
    const DELAY_MILLIS: u64 = 50;

    let mut last_state = read_state();
    let mut color = state_color(&last_state);
    let mut animation: Option<ColorAnimation> = None;

    // Last time we updated the color & intensity
    let mut last_t = scheduler.now();
    let mut theta = 0.0;

    loop {
        let now = scheduler.now();
        let delta_t = now - last_t;
        last_t = now;

        let state = read_state();

        // If the state changed, start a new animation
        if state != last_state {
            let (r_, g_, b_) = state_color(&state);
            let (r, g, b) = color;

            animation = Some(ColorAnimation {
                started_at: now,
                start_color: color,
                diff_color: (r_ - r, g_ - g, b_ - b),
            });
        }

        // Update the color, if necessary
        if let Some(ref anim) = animation {
            // between [0,1], linearly interpolated
            let ratio =
                (now - anim.started_at).to_millis() as f64 / ANIM_DURATION.to_millis() as f64;
            let ratio = libm::fmin(1.0, ratio);

            // Apply the diff, proportionally
            color = (
                anim.start_color.0 + ratio * anim.diff_color.0,
                anim.start_color.1 + ratio * anim.diff_color.1,
                anim.start_color.2 + ratio * anim.diff_color.2,
            );

            // Animation end
            if ratio >= 1.0 {
                animation = None;
            }
        };

        // State specific data
        let (v_min, v_max, period_millis) = match state {
            State::Typing => (0.0, 1.0, 1000.0),
            State::Stopped => (0.3, 0.5, 2000.0),
        };

        // How far we are in one blink (intensity follows a sine wave)
        theta += (delta_t.to_millis() as f64 / period_millis) * TAU;
        let intensity = (v_max - v_min) * (0.5 * libm::sin(theta) + 0.5) + v_min;

        led_channels.set_rgb(
            intensity * color.0,
            intensity * color.1,
            intensity * color.2,
        );
        last_state = state;

        // Wait some time before adjusting color
        scheduler.sleep_ms(DELAY_MILLIS).await;
    }
}

async fn handle_usb<'a>(
    scheduler: &ghostwriter::Scheduler<'a>,
    state: &Mutex<RefCell<State>>,
    rosc: &mut RingOscillator<Enabled>,
) {
    let mut n_written: usize = 0;

    // KeyPRessDelay & InterKeyInterval distributions
    // naming from "Observations on Typing from 136 Million Keystrokes"
    // distributions adapted
    let rand_kprd = Normal::new(50.49, 17.38).unwrap();
    let rand_iki = ChiSquared::new(5.0).unwrap();

    loop {
        debug!("ghostwriter waiting for click");
        // We're stopped and waiting for a click
        scheduler.wait_for_click().await;
        debug!("ghostwriter received click");

        // Button was pressed, so notify the LEDs
        critical_section::with(|cs| state.borrow(cs).replace(State::Typing));

        // Write chars forever (until interrupted)
        let write = async {
            loop {
                let c = {
                    let chr = text::TEXT.as_bytes()[n_written];
                    n_written = (n_written + 1) % text::TEXT.len();
                    chr
                };
                let kprd = rand_kprd.sample(rosc) as u64;

                write_char(scheduler, kprd, c).await;

                let iki = 30 + 10 * rand_iki.sample(rosc) as u64;
                scheduler.sleep_ms(iki).await;
            }
        };

        // Bias towards the click, so that it's always noticed first
        select_biased! {
            () = scheduler.wait_for_click().fuse() => (),
            () = write.fuse() => (),
        }
        debug!("ghostwriter releasing keys");

        // Button was pressed, so release all keys in the keyboard and notify the LEDs
        release_keys();
        critical_section::with(|cs| state.borrow(cs).replace(State::Stopped));
        debug!("ghostwriter output stopped");
    }
}

async fn write_char<'a>(scheduler: &ghostwriter::Scheduler<'a>, kprd: u64, chr: u8) {
    let keycode = text::char_to_keycode(chr);
    write_key(scheduler, kprd, keycode).await;
}

async fn write_key<'a>(scheduler: &ghostwriter::Scheduler<'a>, kprd: u64, keycode: u8) {
    let report_keydown = KeyboardReport {
        modifier: 0,
        reserved: 0,
        leds: 0,
        keycodes: [keycode, 0, 0, 0, 0, 0],
    };
    let _ = ghostwriter::usb::push_hid_report(report_keydown);
    scheduler.sleep_ms(kprd).await;
    release_keys();
}

/// Release all keys on the keyboard
fn release_keys() {
    let report_keyup = KeyboardReport {
        modifier: 0,
        reserved: 0,
        leds: 0,
        keycodes: [0x00, 0, 0, 0, 0, 0],
    };

    let _ = ghostwriter::usb::push_hid_report(report_keyup);
}
