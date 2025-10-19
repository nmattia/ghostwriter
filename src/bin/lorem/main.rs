//! An HID/Keyboard device that types preloaded text.

#![no_std]
#![no_main]

use {defmt_rtt as _, panic_probe as _};

// Locking
use core::cell::RefCell;
use critical_section::Mutex;

// USB Human Interface Device (HID) Class support
use usbd_hid::descriptor::{KeyboardReport, SerializedDescriptor};

use core::f64::consts::TAU;

use defmt::*;
use embassy_executor::Spawner;
use embassy_futures::{join::join, select::select};
use embassy_rp::bind_interrupts;
use embassy_rp::clocks::RoscRng;
use embassy_rp::gpio::{Input, Pull};
use embassy_rp::peripherals::USB;
use embassy_rp::usb::{Driver, InterruptHandler};
use embassy_time::{Duration, Instant, Timer};
use embassy_usb::class::hid;
use embassy_usb::{Builder, Config};

use rand_distr::{ChiSquared, Distribution, Normal};

use ghostwriter::leds;

mod text;

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

type HidWriter<'a> = hid::HidWriter<'a, Driver<'a, USB>, 8>;

// Whether we're typing or not
#[derive(Clone, PartialEq, Eq)]
enum State {
    Typing,
    Stopped,
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    // Create the driver, from the HAL.
    let driver = Driver::new(p.USB, Irqs);

    // Create embassy-usb Config
    let mut config = Config::new(0xc0de, 0xcafe);
    config.manufacturer = Some("Boo-inc");
    config.product = Some("Ghostwriter");
    config.serial_number = Some("0oooo00000");

    // Create embassy-usb DeviceBuilder using the driver and config.
    // It needs some buffers for building the descriptors.
    let mut config_descriptor = [0; 256];
    let mut bos_descriptor = [0; 256];
    let mut msos_descriptor = [0; 256];
    let mut control_buf = [0; 64];

    let mut state = hid::State::new();

    let mut builder = Builder::new(
        driver,
        config,
        &mut config_descriptor,
        &mut bos_descriptor,
        &mut msos_descriptor,
        &mut control_buf,
    );

    // Create classes on the builder.
    let config = embassy_usb::class::hid::Config {
        report_descriptor: KeyboardReport::desc(),
        request_handler: None,
        poll_ms: 60,
        max_packet_size: 64,
    };
    let mut writer = hid::HidWriter::<_, 8>::new(&mut builder, &mut state, config);

    // Build the builder.
    let mut usb = builder.build();

    // Run the USB device.
    let usb_fut = usb.run();

    // Set up the signal pin that will be used to trigger the keyboard.
    let mut signal_pin = Input::new(p.PIN_23, Pull::None);

    // Enable the schmitt trigger to slightly debounce.
    signal_pin.set_schmitt(true);

    // The global state (communication between LEDs & main loop)
    let state: Mutex<RefCell<State>> = Mutex::new(RefCell::new(State::Stopped));

    let led_channels = leds::init_pwm((p.PWM_SLICE1, p.PWM_SLICE2), (p.PIN_18, p.PIN_19, p.PIN_20));

    // Do stuff with the class!
    let handle_usb = handle_usb(&mut writer, signal_pin, &state);
    let handle_leds = handle_leds(&state, led_channels);

    // Run everything concurrently.
    // If we had made everything `'static` above instead, we could do this using separate tasks instead.
    join(usb_fut, join(handle_leds, handle_usb)).await;
}

/// Animate the LEDs based on the state
async fn handle_leds(state: &Mutex<RefCell<State>>, mut led_channels: crate::leds::LEDChannels) {
    let read_state = || critical_section::with(|cs| state.borrow(cs).borrow().clone());

    #[allow(clippy::eq_op)]
    let state_color = |state: &State| match state {
        State::Typing => (1.0 / 1.0, 1.0 / 9.0, 1.0 / 1.0),
        State::Stopped => (1.0 / 3.0, 1.0 / 5.0, 1.0 / 4.0),
    };

    type Color = (f64, f64, f64);
    struct ColorAnimation {
        started_at: Instant, /* timestamp at which animation was started */
        start_color: Color,
        diff_color: Color,
    }

    // Total animation duration
    const ANIM_DURATION: Duration = Duration::from_millis(300);

    // Animation delay (pause between LED adjustments)
    const DELAY_MILLIS: u64 = 50;

    let mut last_state = read_state();
    let mut color = state_color(&last_state);
    let mut animation: Option<ColorAnimation> = None;

    // Last time we updated the color & intensity
    let mut last_t = Instant::now();
    let mut theta = 0.0;

    loop {
        let now = Instant::now();
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
                (now - anim.started_at).as_millis() as f64 / ANIM_DURATION.as_millis() as f64;
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
        theta += (delta_t.as_millis() as f64 / period_millis) * TAU;
        let intensity = (v_max - v_min) * (0.5 * libm::sin(theta) + 0.5) + v_min;

        led_channels.set_rgb(
            intensity * color.0,
            intensity * color.1,
            intensity * color.2,
        );
        last_state = state;

        // Wait some time before adjusting color
        Timer::after_nanos(DELAY_MILLIS * 1000 * 1000).await;
    }
}

async fn handle_usb<'a>(
    writer: &mut HidWriter<'a>,
    mut signal_pin: Input<'a>,
    state: &Mutex<RefCell<State>>,
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
        signal_pin.wait_for_falling_edge().await;
        debug!("ghostwriter received click");

        // Button was pressed, so notify the LEDs
        critical_section::with(|cs| state.borrow(cs).replace(State::Typing));

        // Write chars forever (until interrupted)
        let write = async {
            loop {
                let c = {
                    let chr = text::TEXT[n_written];
                    n_written = (n_written + 1) % text::TEXT.len();
                    chr
                };
                let kprd = rand_kprd.sample(&mut RoscRng) as u64;

                ghostwriter::keyboard::write_ascii_byte(writer, c).await;
                Timer::after_nanos(kprd * 1000 * 1000).await;
                ghostwriter::keyboard::release_keys(writer).await;

                let iki = 30 + 10 * rand_iki.sample(&mut RoscRng) as u64;
                Timer::after_nanos(iki * 1000 * 1000).await;
            }
        };

        let _ = select(signal_pin.wait_for_falling_edge(), write).await;

        debug!("ghostwriter releasing keys");

        // Button was pressed, so release all keys in the keyboard and notify the LEDs
        ghostwriter::keyboard::release_keys(writer).await;
        critical_section::with(|cs| state.borrow(cs).replace(State::Stopped));
        debug!("ghostwriter output stopped");
    }
}
