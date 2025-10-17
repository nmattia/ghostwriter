#![allow(static_mut_refs)]

use panic_halt as _;

// Device specific
use pimoroni_tiny2040 as bsp;

use bsp::hal;
use bsp::hal::{pac, pac::interrupt};

// USB Device support
use usb_device::{class_prelude::*, prelude::*};

// USB Human Interface Device (HID) Class support
use usbd_hid::descriptor::generator_prelude::*;
use usbd_hid::descriptor::KeyboardReport;
use usbd_hid::hid_class::HIDClass;

/// USB

/// The USB Device Driver (shared with the interrupt).
static mut USB_DEVICE: Option<UsbDevice<hal::usb::UsbBus>> = None;

/// The USB Bus Driver (shared with the interrupt).
static mut USB_BUS: Option<UsbBusAllocator<hal::usb::UsbBus>> = None;

/// The USB Human Interface Device Driver (shared with the interrupt).
static mut USB_HID: Option<HIDClass<hal::usb::UsbBus>> = None;

/// Sets up the USB driver. This will set the global USB device.
///
/// IMPORTANT:
///   1. Call this before interrupts are enabled.
///   2. Don't forget to enable the USB interrupt after calling this.
pub fn setup_usb_driver(
    usbctrl_regs: pac::USBCTRL_REGS,
    usbctrl_dpram: pac::USBCTRL_DPRAM,
    usb_clock: hal::clocks::UsbClock,
    resets: &mut pac::RESETS,
) {
    let usb_bus = UsbBusAllocator::new(hal::usb::UsbBus::new(
        usbctrl_regs,
        usbctrl_dpram,
        usb_clock,
        true,
        resets,
    ));

    unsafe {
        // Note (safety): This is safe as interrupts haven't been started yet
        USB_BUS = Some(usb_bus);
    }

    let usb_dev = {
        // Grab a reference to the USB Bus allocator. We are promising to the
        // compiler not to take mutable access to this global variable whilst this
        // reference exists!
        let bus_ref = unsafe { USB_BUS.as_ref().unwrap() };

        // Set up the USB HID Class Device driver
        let usb_hid = HIDClass::new(bus_ref, KeyboardReport::desc(), 60);
        unsafe {
            // Note (safety): This is safe as interrupts haven't been started yet.
            USB_HID = Some(usb_hid);
        }

        // Create a USB device with a fake VID and PID
        UsbDeviceBuilder::new(bus_ref, UsbVidPid(0x16c0, 0x27da))
            .strings(&[StringDescriptors::default()
                .manufacturer("Boo-inc")
                .product("Ghostwriter")
                .serial_number("0oooo00000")])
            .unwrap()
            .device_class(0)
            .build()
    };

    unsafe {
        // Note (safety): This is safe as interrupts haven't been started yet
        USB_DEVICE = Some(usb_dev);
    }
}

/// Submit a new HID report to the USB stack.
///
/// We do this with interrupts disabled (critical_section), to avoid a race hazard
/// with the USB IRQ.
pub fn push_hid_report(report: KeyboardReport) -> Result<usize, usb_device::UsbError> {
    critical_section::with(|_| unsafe {
        // Now interrupts are disabled, grab the global variable and, if
        // available, send it a HID report
        USB_HID.as_mut().map(|hid| hid.push_input(&report))
    })
    .unwrap()
}

/// This function is called whenever the USB Hardware generates an Interrupt
/// Request.
#[interrupt]
unsafe fn USBCTRL_IRQ() {
    // Handle USB request
    let usb_dev = USB_DEVICE.as_mut().unwrap();
    let usb_hid = USB_HID.as_mut().unwrap();
    usb_dev.poll(&mut [usb_hid]);
}
