//! USB Keyboard (HID class) helpers

use embassy_rp::peripherals::USB;
use embassy_rp::usb::Driver;
use embassy_time::{Duration, Timer};
use embassy_usb::class::hid;
use usbd_hid::descriptor::KeyboardReport;

type HidWriter<'a> = hid::HidWriter<'a, Driver<'a, USB>, 8>;

/// Write an ASCII-interpreted byte to the HID device
/// NOTE: If the SHIFT key has to be pressed, a delay is introduced
pub async fn write_ascii_byte<'a>(writer: &mut HidWriter<'a>, chr: u8) {
    let (keycode, shifted) = char_to_keycode(chr);

    if shifted {
        let report_shift = KeyboardReport {
            modifier: 2,
            reserved: 0,
            leds: 0,
            keycodes: [0, 0, 0, 0, 0, 0],
        };
        let _ = writer.write_serialize(&report_shift).await;
        Timer::after(Duration::from_millis(30)).await;
    }

    let report_char = KeyboardReport {
        modifier: 2,
        reserved: 0,
        leds: 0,
        keycodes: [keycode, 0, 0, 0, 0, 0],
    };

    let _ = writer.write_serialize(&report_char).await;
}

/// Map ASCII chars to keyboard (US layout) keycodes. The second tuple element
/// is 'true' if the shift key is pressed.
///
/// See `man ascii / "decimal" set` for ascii codes.
///
/// HID Keycodes from "Keycode by Edward Hage":
/// https://europe1.discourse-cdn.com/arduino/original/4X/1/1/4/114781aa9e26c56002ed8f611a9b3554dc2e0f52.png
/// NOTE: this assumes a US layout
pub fn char_to_keycode(chr: u8) -> (u8, bool) {
    match char::from(chr) {
        'a'..='z' => (chr - b'a' + 4, false),
        'A'..='Z' => (chr - b'A' + 4, true),
        '!' => (30, true),
        '\n' => (40, false),
        ' ' => (44, false),
        '-' => (45, false),
        ':' => (51, true),
        '\'' => (52, false),
        ',' => (54, false),
        '.' => (55, false),
        '>' => (55, true),
        '/' => (56, false),
        '?' => (56, true),

        // If unknown, default to question mark
        _ => (56, true),
    }
}

pub const ALL_KEYS_UP: KeyboardReport = KeyboardReport {
    modifier: 0,
    reserved: 0,
    leds: 0,
    keycodes: [0x00, 0, 0, 0, 0, 0],
};

/// Release all keys on the keyboard
pub async fn release_keys<'a>(writer: &mut HidWriter<'a>) {
    let _ = writer.write_serialize(&ALL_KEYS_UP).await;
}

/// Write an entire string
pub async fn write_str<'a>(writer: &mut HidWriter<'a>, s: &'a str, delay: Duration) {
    let total = s.len();
    let mut ix = 0;
    let bytes = s.as_bytes();

    while ix < total {
        let chr = bytes[ix];

        ix += 1;

        write_ascii_byte(writer, chr).await;

        Timer::after(delay).await;

        release_keys(writer).await;
        Timer::after(delay).await;
    }
}
