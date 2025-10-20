//! An HID/Keyboard device that triggers a Space (short press) or Enter (long press)

#![no_std]
#![no_main]

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

    // Set up the signal pin that will be used to trigger the keyboard.
    let mut signal_pin = Input::new(p.PIN_23, Pull::None);

    // Enable the schmitt trigger to slightly debounce.
    signal_pin.set_schmitt(true);

    let play_fut = play(&mut writer, signal_pin);

    // Run everything concurrently.
    // If we had made everything `'static` above instead, we could do this using separate tasks instead.
    join(usb_fut, play_fut).await;
}

const SPACE: KeyboardReport = KeyboardReport {
    modifier: 0,
    reserved: 0,
    leds: 0,
    keycodes: [44, 0, 0, 0, 0, 0],
};

const ENTER: KeyboardReport = KeyboardReport {
    modifier: 0,
    reserved: 0,
    leds: 0,
    keycodes: [40, 0, 0, 0, 0, 0],
};

async fn play<'a>(writer: &mut HidWriter<'a>, mut signal_pin: Input<'a>) {
    loop {
        signal_pin.wait_for_falling_edge().await;

        if embassy_time::with_timeout(
            Duration::from_millis(600),
            signal_pin.wait_for_rising_edge(),
        )
        .await
        .is_err()
        {
            let _ = writer.write_serialize(&ENTER).await;
            Timer::after(DELAY).await;
            ghostwriter::keyboard::release_keys(writer).await;
            // 1 sec debounce delay
            Timer::after(Duration::from_secs(1)).await;
        } else {
            let _ = writer.write_serialize(&SPACE).await;
            Timer::after(DELAY).await;
            ghostwriter::keyboard::release_keys(writer).await;
            Timer::after(DELAY).await;
        }
    }
}

const DELAY: Duration = Duration::from_millis(30);
