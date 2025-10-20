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
use embassy_time::{Duration, Instant, Ticker, Timer};
use embassy_usb::class::hid;

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
    let mut config = embassy_usb::Config::new(0xc0de, 0xcafe);
    config.manufacturer = Some("Boo-inc");
    config.product = Some("Ghostwriter");
    config.serial_number = Some("0oooo00000");

    let mut hid_state = hid::State::new(); // HID state

    // Create embassy-usb DeviceBuilder using the driver and config.
    // It needs some buffers for building the descriptors.
    let mut config_descriptor = [0; 256];
    let mut bos_descriptor = [0; 256];
    let mut msos_descriptor = [0; 256];
    let mut control_buf = [0; 64];
    let mut builder = embassy_usb::Builder::new(
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

    let mut writer = hid::HidWriter::<_, 8>::new(&mut builder, &mut hid_state, config);

    // Build the builder and run the device.
    let mut usb = builder.build();
    let usb_fut = usb.run();

    // Set up the signal pin that will be used to trigger the keyboard.
    let signal_pin = {
        let mut signal_pin = Input::new(p.PIN_23, Pull::None);
        // Enable the schmitt trigger to slightly debounce.
        signal_pin.set_schmitt(true);
        signal_pin
    };

    let led_channels = leds::init_pwm((p.PWM_SLICE1, p.PWM_SLICE2), (p.PIN_18, p.PIN_19, p.PIN_20));

    // The global state (communication between LEDs & main loop)
    let state: Mutex<RefCell<State>> = Mutex::new(RefCell::new(State::Stopped));

    // Lorem-specific functions
    let handle_usb = handle_usb(&mut writer, signal_pin, &state);
    let handle_leds = handle_leds(&state, led_channels);

    // Run everything concurrently.
    // If we had made everything `'static` above instead, we could do this using separate tasks instead.
    join(usb_fut, join(handle_leds, handle_usb)).await;
}

/// Animate the LEDs based on the state
async fn handle_leds(state: &Mutex<RefCell<State>>, mut led_channels: crate::leds::LEDChannels) {
    let read_state = || critical_section::with(|cs| state.borrow(cs).borrow().clone());

    // State specific data
    #[allow(clippy::eq_op)]
    let state_data = |state: &State| match state {
        State::Typing => ((1.0 / 1.0, 1.0 / 9.0, 1.0 / 1.0), 0.0, 1.0, 1000.0),
        State::Stopped => ((1.0 / 3.0, 1.0 / 5.0, 1.0 / 4.0), 0.3, 0.5, 2000.0),
    };

    // Update the LEDs every 50ms
    let mut ticker = Ticker::every(Duration::from_millis(50));

    loop {
        let (color, v_min, v_max, period_millis) = state_data(&read_state());

        // How far we are in one blink (intensity follows a sine wave)
        let theta = (Instant::now().as_millis() as f64 / period_millis) * TAU;
        let intensity = (v_max - v_min) * (0.5 * libm::sin(theta) + 0.5) + v_min;

        led_channels.set_rgb(
            intensity * color.0,
            intensity * color.1,
            intensity * color.2,
        );

        // Wait some time before adjusting color
        ticker.next().await;
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
                let iki = 30 + 10 * rand_iki.sample(&mut RoscRng) as u64;

                ghostwriter::keyboard::write_ascii_byte(writer, c).await;
                Timer::after(Duration::from_millis(kprd)).await;
                ghostwriter::keyboard::release_keys(writer).await;
                Timer::after(Duration::from_millis(iki)).await;
            }
        };

        // Write until the button is pressed
        let _ = select(write, signal_pin.wait_for_falling_edge()).await;
        debug!("ghostwriter releasing keys");

        // Button was pressed, so release all keys in the keyboard and notify the LEDs
        ghostwriter::keyboard::release_keys(writer).await;
        critical_section::with(|cs| state.borrow(cs).replace(State::Stopped));
        debug!("ghostwriter output stopped");
    }
}
