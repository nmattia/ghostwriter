// Module driving LEDs through PWM.
//
// One PWM cycle is driven via the main 133Mhz clock, via a divider. Then,
// each cycle contains N cycle ticks. Out of a cycle, a LED is OFF while on
// "duty" (because the tiny 2040 LEDs are active low) and hence the OFF time
// is dictated by the "duty" or "duty cycle" (from 0 to u16::MAX or through
// helpers).
//
// See RP2040 datasheet Section 4.5.2.1 (Pulse Width Modulation)

use embassy_rp::pwm;
use embassy_rp::pwm::SetDutyCycle;
use embassy_rp::Peri;

use embassy_rp::peripherals::{PIN_18, PIN_19, PIN_20, PWM_SLICE1, PWM_SLICE2};

// clock divider: 133Mhz / 256 ~= 500kHz
const PWM_DIV: u8 = u8::MAX;

// Period in ticks: 512 ticks -> 500kHz / 512 ~= 1kHz (i.e. 1PWM cycle ~= 1ms)
const PWM_TOP: u16 = 512;

// PWM channels for each pin, refer to RGB led pins here:
//  https://shop.pimoroni.com/products/tiny-2040
// and RP2040 datasheet PWM channels(4.5.2 Programmer's Model)
pub struct LEDChannels {
    // GPIO 18 -> PWM 1A
    red: pwm::PwmOutput<'static>,
    // GPIO 19 -> PWM 1B
    green: pwm::PwmOutput<'static>,
    // GPIO 20 -> PWM 2A
    blue: pwm::PwmOutput<'static>,
}

impl LEDChannels {
    // NOTE: duty "on/off" is inverted because LEDs are active low
    // NOTE: we 'unwrap' because the error is actually Infallible

    pub fn set_rgb(&mut self, r: f64, g: f64, b: f64) {
        let convert = |v: f64| ((1.0 - v) * 100.0) as u8;
        self.red.set_duty_cycle_percent(convert(r)).unwrap();
        self.green.set_duty_cycle_percent(convert(g)).unwrap();
        self.blue.set_duty_cycle_percent(convert(b)).unwrap();
    }
}

type LEDPins = (
    Peri<'static, PIN_18>,
    Peri<'static, PIN_19>,
    Peri<'static, PIN_20>,
);

pub fn init_pwm(
    slices: (Peri<'static, PWM_SLICE1>, Peri<'static, PWM_SLICE2>),
    led_pins: LEDPins,
) -> LEDChannels {
    // Configure PWM
    let (slice1, slice2) = slices;

    let mut c = pwm::Config::default();

    // Set up cycle duration
    c.top = PWM_TOP;
    c.divider = PWM_DIV.into();

    let slice1_ab = pwm::Pwm::new_output_ab(slice1, led_pins.0, led_pins.1, c.clone());
    let slice1_ab = slice1_ab.split();

    let red = slice1_ab.0.unwrap();
    let green = slice1_ab.1.unwrap();
    let blue = pwm::Pwm::new_output_a(slice2, led_pins.2, c.clone())
        .split()
        .0
        .unwrap();

    LEDChannels { red, green, blue }
}
