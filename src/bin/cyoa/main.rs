//! An HID/Keyboard device that plays a Choose Your Own Adventure game

#![no_std]
#![no_main]

use core::str;
use {defmt_rtt as _, panic_probe as _};

// USB Human Interface Device (HID) Class support
use usbd_hid::descriptor::{KeyboardReport, SerializedDescriptor};

use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Input, Pull};
use embassy_rp::peripherals::USB;
use embassy_rp::usb::{Driver, InterruptHandler};
use embassy_time::{Duration, Timer};
use embassy_usb::class::hid;
use embassy_usb::{Builder, Config};

use ghostwriter::keyboard::write_str;
use ghostwriter::leds;

const STORY: &str = include_str!("ghostwriter.html");

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

type HidWriter<'a> = hid::HidWriter<'a, Driver<'a, USB>, 8>;

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

    let led_slices = leds::init_pwm((p.PWM_SLICE1, p.PWM_SLICE2), (p.PIN_18, p.PIN_19, p.PIN_20));

    let leds_signal = leds::Signal::new();
    let leds_fut = leds::animate_leds(&leds_signal, led_slices);

    // Set up the signal pin that will be used to trigger the keyboard.
    let mut signal_pin = Input::new(p.PIN_23, Pull::None);

    // Enable the schmitt trigger to slightly debounce.
    signal_pin.set_schmitt(true);

    let play_fut = play(&mut writer, signal_pin, &leds_signal);
    let app_fut = join(play_fut, leds_fut);

    join(usb_fut, app_fut).await;
}

const SHIFT: KeyboardReport = KeyboardReport {
    modifier: 2,
    reserved: 0,
    leds: 0,
    keycodes: [0, 0, 0, 0, 0, 0],
};
const CARET: KeyboardReport = KeyboardReport {
    modifier: 2,
    reserved: 0,
    leds: 0,
    keycodes: [55, 0, 0, 0, 0, 0],
};
const RIGHT: KeyboardReport = KeyboardReport {
    modifier: 2,
    reserved: 0,
    leds: 0,
    keycodes: [79, 0, 0, 0, 0, 0],
};
const LEFT: KeyboardReport = KeyboardReport {
    modifier: 2,
    reserved: 0,
    leds: 0,
    keycodes: [80, 0, 0, 0, 0, 0],
};
const DOWN: KeyboardReport = KeyboardReport {
    modifier: 0,
    reserved: 0,
    leds: 0,
    keycodes: [81, 0, 0, 0, 0, 0],
};
const UP: KeyboardReport = KeyboardReport {
    modifier: 0,
    reserved: 0,
    leds: 0,
    keycodes: [82, 0, 0, 0, 0, 0],
};
const SPACE: KeyboardReport = KeyboardReport {
    modifier: 0,
    reserved: 0,
    leds: 0,
    keycodes: [44, 0, 0, 0, 0, 0],
};

/// Short press, going through menus
const PRESSED_ANIMATION: leds::Animation = leds::Animation {
    #[allow(clippy::eq_op)]
    color: (1.0 / 3.0, 1.0 / 5.0, 1.0 / 4.0),
    bounds: (0.3, 1.0),
    peak_after: Duration::from_millis(200),
    loop_after: None,
};

/// The ghostwriter is typing
const TYPING_ANIMATION: leds::Animation = leds::Animation {
    #[allow(clippy::eq_op)]
    color: (1.0 / 1.0, 1.0 / 9.0, 1.0 / 1.0),
    bounds: (0.0, 1.0),
    peak_after: Duration::from_millis(100),
    loop_after: Some(Duration::from_millis(400)),
};

async fn play<'a>(
    writer: &mut HidWriter<'a>,
    mut signal_pin: Input<'a>,
    leds_signal: &leds::Signal,
) {
    let start_passage_id = twine::find_start_passage_id(STORY);

    let mut passage = twine::find_passage_text_by_id(STORY, start_passage_id);

    signal_pin.wait_for_falling_edge().await;

    loop {
        // Loop until there are no more links to other passages
        let link_section_start = match passage.find("[[") {
            None => break,
            Some(l) => l,
        };

        leds_signal.signal(TYPING_ANIMATION);

        // If a link is found, write the passage until the link
        write_str(writer, &passage[..link_section_start], DELAY).await;

        // Then offer the next passage selection
        let res = select_passage_link(writer, &mut signal_pin, leds_signal, passage).await;
        passage = twine::find_passage_text_by_name(STORY, res);
    }
}

const DELAY: Duration = Duration::from_millis(30);

async fn select_passage_link<'a>(
    writer: &mut HidWriter<'a>,
    signal_pin: &mut Input<'a>,
    leds_signal: &leds::Signal,
    link_section: &'a str,
) -> &'a str {
    let n_links = twine::get_n_links(link_section);

    // Shenanigans to list the various links to other passages.

    // First write all the links with some padding on the left:
    //
    // |   First Option
    // |   Second Option
    for ix in 0..n_links {
        let link_data = twine::get_link_data(link_section, ix);
        write_str(writer, "   ", DELAY).await;
        write_str(writer, link_data.label, DELAY).await;
        write_str(writer, "\n", DELAY).await;
    }

    // Then the last newline will place the cursor on the leftmost position. From
    // there we move up and insert a caret in front of the first option.
    // | > First Option
    // |   Second Option
    for _ in 0..n_links {
        let _ = writer.write_serialize(&UP).await;
        Timer::after(DELAY).await;

        ghostwriter::keyboard::release_keys(writer).await;
        Timer::after(DELAY).await;
    }

    for _ in 0..2 {
        let _ = writer.write_serialize(&RIGHT).await;
        Timer::after(DELAY).await;
        ghostwriter::keyboard::release_keys(writer).await;
        Timer::after(DELAY).await;
    }

    // CARET
    let _ = writer.write_serialize(&SHIFT).await;
    Timer::after(DELAY).await;
    let _ = writer.write_serialize(&LEFT).await;
    Timer::after(DELAY).await;
    let _ = writer.write_serialize(&SHIFT).await;
    Timer::after(DELAY).await;
    let _ = writer.write_serialize(&CARET).await;
    Timer::after(DELAY).await;
    ghostwriter::keyboard::release_keys(writer).await;
    Timer::after(DELAY).await;

    // Finally, whenever there's a short press, highlight the caret, replace
    // it with a space, move the cursor to the next option (possible looping
    // to the first) and replace the space there with a caret.

    let mut target = 0;
    let mut current = 0;

    leds_signal.signal(PRESSED_ANIMATION); // basically stop the typing animation
    loop {
        signal_pin.wait_for_falling_edge().await;

        leds_signal.signal(PRESSED_ANIMATION);

        if embassy_time::with_timeout(
            Duration::from_millis(600),
            signal_pin.wait_for_rising_edge(),
        )
        .await
        .is_err()
        {
            break;
        }

        target = (target + 1) % (n_links as i32);

        // REPLACE
        let _ = writer.write_serialize(&SHIFT).await;
        Timer::after(DELAY).await;
        let _ = writer.write_serialize(&LEFT).await;
        Timer::after(DELAY).await;
        ghostwriter::keyboard::release_keys(writer).await;
        Timer::after(DELAY).await;
        let _ = writer.write_serialize(&SPACE).await;
        Timer::after(DELAY).await;
        ghostwriter::keyboard::release_keys(writer).await;
        Timer::after(DELAY).await;

        // Bookkeeping for cursor position
        let delta: i32 = target - current; // > 0 if lower
        let key = if delta > 0 { DOWN } else { UP };
        let delta = delta.abs();

        for _ in 0..delta {
            let _ = writer.write_serialize(&key).await;
            Timer::after(DELAY).await;
            ghostwriter::keyboard::release_keys(writer).await;
            Timer::after(DELAY).await;
        }

        current = target;

        // CARET
        let _ = writer.write_serialize(&SHIFT).await;
        Timer::after(DELAY).await;
        let _ = writer.write_serialize(&LEFT).await;
        Timer::after(DELAY).await;
        let _ = writer.write_serialize(&SHIFT).await;
        Timer::after(DELAY).await;
        let _ = writer.write_serialize(&CARET).await;
        Timer::after(DELAY).await;
        ghostwriter::keyboard::release_keys(writer).await;
        Timer::after(DELAY).await;
    }

    let n_links: i32 = n_links.try_into().unwrap();

    let delta = n_links - TryInto::<i32>::try_into(current).unwrap();
    for _ in 0..delta {
        let _ = writer.write_serialize(&DOWN).await;
        Timer::after(DELAY).await;
        ghostwriter::keyboard::release_keys(writer).await;
        Timer::after(DELAY).await;
    }

    ghostwriter::keyboard::release_keys(writer).await;
    Timer::after(DELAY).await;

    // some line returns to give room to the next passage
    ghostwriter::keyboard::write_ascii_byte(writer, 10).await;
    Timer::after(DELAY).await;
    ghostwriter::keyboard::write_ascii_byte(writer, 10).await;
    Timer::after(DELAY).await;
    ghostwriter::keyboard::write_ascii_byte(writer, 10).await;
    Timer::after(DELAY).await;

    // Finally return the name of the passage to go to
    let link_data = twine::get_link_data(link_section, current.try_into().unwrap());
    link_data.target
}
