use core::pin::Pin;
use futures::task::{Context, Poll};

use futures::Future;

// Device specific
use pimoroni_tiny2040 as bsp;

use bsp::hal::{gpio, pac::interrupt};

// Locking
use core::cell::RefCell;
use critical_section::Mutex;

use core::option::Option;
use core::option::Option::{None, Some};

/// The pin used as an input button. Global value set once by the setup, and then read
/// once (stolen, taken) from the button interrupt handler.
type ButtonPin = gpio::Pin<gpio::bank0::Gpio23, gpio::FunctionSio<gpio::SioInput>, gpio::PullUp>;
static _BUTTON_PIN: Mutex<RefCell<Option<ButtonPin>>> = Mutex::new(RefCell::new(None));

/// Global state shared between the eventloop and the button interrupt handler.
static BUTTON_DOWN: Mutex<RefCell<Option<Keypress>>> = Mutex::new(RefCell::new(None));

// Input

// Result of a keypress, either down (pressed) or up (released)
pub enum Keypress {
    Up,
    Down,
}

/// Async/await waiting for user input (button press)
pub struct Input {}

impl Future for Input {
    type Output = Keypress;

    fn poll(self: Pin<&mut Self>, _ctx: &mut Context<'_>) -> Poll<Self::Output> {
        // Check if button has been pressed
        let was_pressed = critical_section::with(|cs| BUTTON_DOWN.borrow(cs).replace(None));

        // If the button was pressed, we're ready to continue
        if let Some(kp) = was_pressed {
            Poll::Ready(kp)
        } else {
            Poll::Pending
        }
    }
}

/// BUTTON

pub fn setup_button(button_pin: ButtonPin) {
    // Trigger on the 'falling edge' of the input pin.
    // This will happen as the button is being pressed
    button_pin.set_interrupt_enabled(gpio::Interrupt::EdgeLow, true);
    button_pin.set_interrupt_enabled(gpio::Interrupt::EdgeHigh, true);

    critical_section::with(|cs| {
        _BUTTON_PIN.borrow(cs).replace(Some(button_pin));
        BUTTON_DOWN.borrow(cs).replace(None);
    });
}

#[interrupt]
fn IO_IRQ_BANK0() {
    // The `#[interrupt]` attribute covertly converts this to `&'static mut Option<...>`
    static mut BUTTON_PIN: Option<ButtonPin> = None;

    // This is one-time lazy initialisation. We steal the global interrupt variables.
    if BUTTON_PIN.is_none() {
        critical_section::with(|cs| {
            *BUTTON_PIN = _BUTTON_PIN.borrow(cs).take();
        });
    }

    if let Some(button) = BUTTON_PIN {
        if button.interrupt_status(gpio::Interrupt::EdgeLow) {
            button.clear_interrupt(gpio::Interrupt::EdgeLow);
            critical_section::with(|cs| {
                BUTTON_DOWN.borrow(cs).replace(Some(Keypress::Down));
            });
        } else if button.interrupt_status(gpio::Interrupt::EdgeHigh) {
            button.clear_interrupt(gpio::Interrupt::EdgeHigh);
            critical_section::with(|cs| {
                BUTTON_DOWN.borrow(cs).replace(Some(Keypress::Up));
            });
        }
    }
}
