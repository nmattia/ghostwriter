use crate::Instant;
use core::pin::Pin;
use futures::task::{Context, Poll};

// Device specific
use bsp::hal;
use pimoroni_tiny2040 as bsp;

use futures::Future;

use bsp::hal::pac::interrupt;
use bsp::hal::timer::Alarm;
use core::cell::RefCell;
use critical_section::Mutex;

use hal::fugit::MicrosDurationU32;

/// When the next wakeup is scheduled (if any)
static NEXT_WAKEUP: Mutex<RefCell<Option<Instant>>> = Mutex::new(RefCell::new(None));

/// The first alarm (see rp2040 datasheet chapter 4.6)
static ALARM0: Mutex<RefCell<Option<hal::timer::Alarm0>>> = Mutex::new(RefCell::new(None));

/// The "epoch", i.e. t0 when the board booted up
pub static BOOT_TIME: Instant = Instant::from_ticks(0);

// Async/await sleep
pub struct Sleep<'a> {
    pub target: Instant,
    pub timer: &'a hal::Timer,
}

impl Future for Sleep<'_> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _ctx: &mut Context<'_>) -> Poll<Self::Output> {
        let now: Instant = BOOT_TIME + self.timer.get_counter().duration_since_epoch();

        if self.target < now {
            Poll::Ready(())
        } else {
            // If we're not ready, then sleep until either the next scheduled wake up or (if
            // earlier) our expected wake up.
            critical_section::with(|cs| {
                NEXT_WAKEUP.borrow(cs).replace_with(|next_wakeup| {
                    let next_wakeup = next_wakeup
                        .map(|next_target| core::cmp::min(self.target, next_target))
                        .unwrap_or(self.target);
                    Some(next_wakeup)
                });
            });

            Poll::Pending
        }
    }
}

/// Setup the sleep variables
pub fn setup_sleep(mut alarm0: hal::timer::Alarm0) {
    alarm0.enable_interrupt();
    let _ = alarm0.schedule(MicrosDurationU32::secs(1));

    critical_section::with(|cs| ALARM0.borrow(cs).replace(Some(alarm0)));
}

/// Make sure the alarm will ring latest at "NEXT_WAKEUP"
pub fn wind_alarm(cs: critical_section::CriticalSection<'_>) {
    let next_wakeup = NEXT_WAKEUP.borrow(cs).replace(None);

    if let Some(next_wakeup) = next_wakeup {
        let mut alarm0 = ALARM0.borrow(cs).borrow_mut();
        let alarm0 = alarm0.as_mut().unwrap();
        let _ = alarm0.schedule_at(next_wakeup);
    }
}

#[interrupt]
fn TIMER_IRQ_0() {
    // Clear the interrupt and return. The only point in this is waking up the cortex
    // from wfi.
    critical_section::with(|cs| {
        let mut alarm0 = ALARM0.borrow(cs).borrow_mut();
        let alarm0 = alarm0.as_mut().unwrap();
        alarm0.clear_interrupt();
    })
}
