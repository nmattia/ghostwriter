use bsp::hal::pwm;
use bsp::hal::pwm::Slice;
use bsp::hal::{
    gpio,
    gpio::bank0::{Gpio18, Gpio19, Gpio20},
    gpio::{FunctionNull, PullDown},
    pac,
};
use pimoroni_tiny2040 as bsp;

// Module driving LEDs through PWM.
//
// One PWM cycle is driven via the main 133Mhz clock, via a divider. Then,
// each cycle contains N cycle ticks. Out of a cycle, a LED is OFF while on
// "duty" (because the tiny 2040 LEDs are active low) and hence the OFF time
// is dictated by the "duty" or "duty cycle" (from 0 to u16::MAX or through
// helpers).
//
// See RP2040 datasheet Section 4.5.2.1 (Pulse Width Modulation)

// GPIO traits
use embedded_hal::pwm::SetDutyCycle;

// clock divider: 133Mhz / 256 ~= 500kHz
const PWM_DIV: u8 = u8::MAX;

// Period in ticks: 512 ticks -> 500kHz / 512 ~= 1kHz (i.e. 1PWM cycle ~= 1ms)
const PWM_TOP: u16 = 512;

// PWM channels for each pin, refer to RGB led pins here:
//  https://shop.pimoroni.com/products/tiny-2040
// and RP2040 datasheet PWM channels(4.5.2 Programmer's Model)
pub struct LEDChannels {
    // GPIO 18 -> PWM 1A
    red: pwm::Channel<Slice<pwm::Pwm1, pwm::FreeRunning>, pwm::A>,
    // GPIO 19 -> PWM 1B
    green: pwm::Channel<Slice<pwm::Pwm1, pwm::FreeRunning>, pwm::B>,
    // GPIO 20 -> PWM 2A
    blue: pwm::Channel<Slice<pwm::Pwm2, pwm::FreeRunning>, pwm::A>,
}

impl LEDChannels {
    // NOTE: duty "on/off" is inverted because LEDs are active low
    // NOTE: we 'unwrap' because the error is actually Infallibe

    pub fn green(&mut self) {
        self.red.set_duty_cycle_fully_on().unwrap();
        self.green.set_duty_cycle_fully_off().unwrap();
        self.blue.set_duty_cycle_fully_on().unwrap();
    }

    pub fn red(&mut self) {
        self.red.set_duty_cycle_fully_off().unwrap();
        self.green.set_duty_cycle_fully_on().unwrap();
        self.blue.set_duty_cycle_fully_on().unwrap();
    }
}

type LEDPins = (
    gpio::Pin<Gpio18, FunctionNull, PullDown>,
    gpio::Pin<Gpio19, FunctionNull, PullDown>,
    gpio::Pin<Gpio20, FunctionNull, PullDown>,
);

pub fn init_pwm(pwm: pac::PWM, resets: &mut pac::RESETS, led_pins: LEDPins) -> LEDChannels {
    // Configure PWM
    let pwm_slices = pwm::Slices::new(pwm, resets);
    let mut pwm1 = pwm_slices.pwm1;
    let mut pwm2 = pwm_slices.pwm2;

    // Set up cycle duration
    pwm1.set_div_int(PWM_DIV);
    pwm2.set_div_int(PWM_DIV);
    pwm1.set_top(PWM_TOP);
    pwm2.set_top(PWM_TOP);

    // NOTE: we 'unwrap' because the error is actually Infallibe
    pwm1.channel_a.set_duty_cycle_fully_on().unwrap();
    pwm1.channel_b.set_duty_cycle_fully_on().unwrap();
    pwm2.channel_a.set_duty_cycle_fully_on().unwrap();

    pwm1.channel_a.output_to(led_pins.0);
    pwm1.channel_b.output_to(led_pins.1);
    pwm2.channel_a.output_to(led_pins.2);

    // Enable as late as possible to avoid flashing
    pwm1.enable();
    pwm2.enable();

    LEDChannels {
        red: pwm1.channel_a,
        green: pwm1.channel_b,
        blue: pwm2.channel_a,
    }
}
