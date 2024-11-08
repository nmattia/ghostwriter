use crate::Keypress;

// Device specific
use pimoroni_tiny2040 as bsp;

use bsp::hal;

use crate::input;
use crate::sleep;

// Time related types
pub type Duration = hal::fugit::Duration<u64, 1, 1000000>;
pub type Instant = hal::fugit::Instant<u64, 1, 1000000>;

// Wrapper around timer so that poll knows the time
pub struct Scheduler<'a> {
    timer: &'a hal::Timer,
}

impl<'a> Scheduler<'a> {
    pub fn new(timer: &'a hal::Timer) -> Self {
        Scheduler { timer }
    }

    pub fn sleep_ms(&self, v: u64) -> sleep::Sleep<'a> {
        let now: Instant = sleep::BOOT_TIME + self.timer.get_counter().duration_since_epoch();
        let delta: Duration = Duration::millis(v);
        sleep::Sleep {
            target: now + delta,
            timer: self.timer,
        }
    }

    pub fn wait_for_press(&self) -> input::Input {
        input::Input {}
    }

    pub async fn wait_for_click(&self) {
        // Loop until a meaningful sequence of Keydown - Keyup occurs.
        //
        // If the sequence happens too fast, pretend this current press didn't happen.
        loop {
            // Wait for press down
            match self.wait_for_press().await {
                Keypress::Up => continue,
                Keypress::Down => {}
            }
            let pressed_at = self.now();

            // Wait for press up
            match self.wait_for_press().await {
                Keypress::Up => {}
                Keypress::Down => continue, // Two key downs, something is off; retry
            }
            let released_at = self.now();
            if (released_at - pressed_at).to_millis() < 50 {
                // Anything this fast we pretend didn't happen
                continue;
            }

            break;
        }
    }

    pub fn now(&self) -> Duration {
        self.timer.get_counter().duration_since_epoch()
    }
}
