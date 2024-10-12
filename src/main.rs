//! An HID/Keyboard device that types preloaded text.

#![no_std]
#![no_main]

use bsp::entry;
use embedded_hal::digital::{InputPin, OutputPin};
use panic_halt as _;

// Device specific
use pimoroni_tiny2040 as bsp;

use bsp::hal;
use bsp::hal::{clocks::Clock, pac, pac::interrupt};

// USB Device support
use usb_device::{class_prelude::*, prelude::*};

// USB Human Interface Device (HID) Class support
use usbd_hid::descriptor::generator_prelude::*;
use usbd_hid::descriptor::KeyboardReport;
use usbd_hid::hid_class::HIDClass;

/// The USB Device Driver (shared with the interrupt).
static mut USB_DEVICE: Option<UsbDevice<hal::usb::UsbBus>> = None;

/// The USB Bus Driver (shared with the interrupt).
static mut USB_BUS: Option<UsbBusAllocator<hal::usb::UsbBus>> = None;

/// The USB Human Interface Device Driver (shared with the interrupt).
static mut USB_HID: Option<HIDClass<hal::usb::UsbBus>> = None;

/// Entry point to our bare-metal application.
///
/// The `#[entry]` macro ensures the Cortex-M start-up code calls this function
/// as soon as all global variables are initialised.
#[entry]
fn main() -> ! {
    // Grab our singleton objects
    let mut pac = pac::Peripherals::take().unwrap();

    // Set up the watchdog driver - needed by the clock setup code
    let mut watchdog = hal::Watchdog::new(pac.WATCHDOG);

    // Configure the clocks
    //
    // The default is to generate a 125 MHz system clock
    let clocks = hal::clocks::init_clocks_and_plls(
        bsp::XOSC_CRYSTAL_FREQ,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    let sio = hal::Sio::new(pac.SIO);
    let pins = bsp::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    // Our button input
    let mut button_pin = pins.bootsel.into_pull_up_input();

    // Set up the USB driver
    let usb_bus = UsbBusAllocator::new(hal::usb::UsbBus::new(
        pac.USBCTRL_REGS,
        pac.USBCTRL_DPRAM,
        clocks.usb_clock,
        true,
        &mut pac.RESETS,
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

        // Set up the USB HID Class Device driver, providing Mouse Reports
        let usb_hid = HIDClass::new(bus_ref, KeyboardReport::desc(), 60);
        unsafe {
            // Note (safety): This is safe as interrupts haven't been started yet.
            USB_HID = Some(usb_hid);
        }

        // Create a USB device with a fake VID and PID
        UsbDeviceBuilder::new(bus_ref, UsbVidPid(0x16c0, 0x27da))
            .strings(&[StringDescriptors::default()
                .manufacturer("Noe's ghostwrtier")
                .product("Up-down-up-down")
                .serial_number("NOPE")])
            .unwrap()
            .device_class(0)
            .build()
    };
    unsafe {
        // Note (safety): This is safe as interrupts haven't been started yet
        USB_DEVICE = Some(usb_dev);
    }

    unsafe {
        // Enable the USB interrupt
        pac::NVIC::unmask(hal::pac::Interrupt::USBCTRL_IRQ);
    };
    let core = pac::CorePeripherals::take().unwrap();
    let mut delay = cortex_m::delay::Delay::new(core.SYST, clocks.system_clock.freq().to_Hz());

    let mut on = false;
    let mut was_pressed = false;
    let mut n_written: usize = 0;

    // Initialization is done, turn on a green light
    let mut led_green = pins.led_green.into_push_pull_output();
    led_green.set_low().unwrap();
    let mut led_red = pins.led_red.into_push_pull_output();
    led_red.set_high().unwrap();
    let mut led_blue = pins.led_blue.into_push_pull_output();
    led_blue.set_high().unwrap();

    // Move the cursor up and down every 200ms
    loop {
        // TODO: use interrupt
        let is_pressed = button_pin.is_low().unwrap();

        if was_pressed && !is_pressed {
            on = !on;
        }

        was_pressed = is_pressed;

        let c = if !on {
            0x00
        } else {
            let chr = TEXT.as_bytes()[n_written];
            let keycode = char_to_keycode(chr);
            n_written = (n_written + 1) % TEXT.len();
            keycode
        };

        let rep_down = KeyboardReport {
            modifier: 0,
            reserved: 0,
            leds: 0,
            keycodes: [c, 0, 0, 0, 0, 0],
        };

        push_hid_report(rep_down).ok().unwrap_or(0);

        delay.delay_ms(50);
        let rep_up = KeyboardReport {
            modifier: 0,
            reserved: 0,
            leds: 0,
            keycodes: [0x00, 0, 0, 0, 0, 0],
        };
        push_hid_report(rep_up).ok().unwrap_or(0);
        delay.delay_ms(100);
    }
}

/// Text (typing) related

/// Convert an ASCII char code to a keyboard keycode
fn char_to_keycode(chr: u8) -> u8 {
    if chr >= 97 && chr <= 122 {
        chr - 97 + 4
    } else if chr == 44 {
        54
    } else if chr == 46 {
        55
    } else if chr == 32 {
        44
    } else if chr == 10 {
        40
    } else if chr == 39 {
        52
    } else if chr == 33 {
        51 // fake exlamation mark
    } else if chr == 58 {
        51 // fake colon (:)
    } else {
        0
    }
}

const TEXT: &str = "
chere mobiliere,

hier soir, tout semblait parfait. j'avais organise un diner avec mes amis, tout etait pret. mais alors que je sortais le gratin du four, j'ai trebuche sur le tapis... le plat s'est envole, et tout s'est renverse sur le sol, y compris mon tapis prefere. heureusement, avec vous, le nettoyage a ete rapide et efficace. merci de m'avoir aide a sauver ma soiree.

chere mobiliere,

ce matin, en me reveillant, je me suis dit que c'etait une belle journee pour une balade en velo. tout se passait bien jusqu'a ce qu'un ecureuil decide de traverser devant moi. pour l'eviter, j'ai freine brusquement et me suis retrouve par terre, avec mon velo en morceaux. heureusement, vous avez ete la pour reparer rapidement mon velo, et l'ecureuil s'en est sorti indemne.

chere mobiliere,

c'etait un jour comme un autre au bureau, jusqu'a ce que je renverse mon verre d'eau sur l'imprimante. l'appareil a fait un drole de bruit et puis plus rien. la panique s'est installee, surtout avec tous les documents importants a imprimer pour une reunion. heureusement, vous etes venus a la rescousse, et en un rien de temps, l'imprimante etait remplacee. merci pour votre rapidite, vous avez sauve ma journee.

chere mobiliere,

hier, j'avais enfin trouve le temps de laver ma voiture. apres une heure de travail acharné, elle brillait comme jamais. mais juste apres avoir termine, une nuée de pigeons est passee au-dessus de moi... et la voiture. heureusement, vous avez ete la pour m'aider a faire le necessaire. merci, vraiment.


chere mobiliere,

ce week-end, j'ai decide de monter un meuble tout seul, sans l'aide des instructions. apres quelques heures de lutte, je me suis retrouve avec une etagere bancale et une vis mysterieuse en trop. le meuble s'est effondre dans la minute. heureusement, vous avez su prendre les choses en main. merci pour votre patience.

chere mobiliere,

c'etait une belle journee de barbecue entre amis. mais lorsque j'ai voulu retourner les brochettes, la grille m'a echappe des mains et tout s'est retrouve par terre. adieu le dejeuner ! heureusement, grace a vous, nous avons pu recommencer sans souci. merci d'avoir sauve notre barbecue.


chere mobiliere,

hier soir, alors que je voulais prendre un bain relaxant, j'ai laissé le robinet ouvert un peu trop longtemps. resultat : une salle de bain inondee, avec de l'eau partout. heureusement, vous avez ete la pour m'aider a reparer les degats. merci d'avoir sauve ma soiree de detente.

chere mobiliere,

l'autre jour, en sortant de la douche, j'ai realise que mon peignoir etait tombe du porte-serviettes. j'ai du me depecher de le ramasser en courant dans l'appartement, en esperant que personne ne passe devant la fenetre. heureusement, tout s'est bien termine, et vous m'avez aide a installer un nouveau porte-serviettes plus solide. merci encore une fois.


chere mobiliere,

ce matin, j'ai voulu prendre un petit-dejeuner au lit pour une fois. mais en voulant me recoucher, j'ai renverse tout mon cafe sur les draps et sur moi-meme. entre le lit trempe et mon pyjama, la journee commencait mal. heureusement, grace a vous, tout a ete nettoye rapidement. merci de m'avoir aide a retablir l'ordre.

chere mobiliere,

hier soir, j'avais prevu une soiree romantique a la maison. tout etait pret : bougies, musique douce... mais en voulant allumer la cheminee pour parfaire l'ambiance, j'ai mal géré et de la fumee a envahi toute la pièce. soiree ratee, mais heureusement, vous avez su nous aider a aerer et nettoyer tout ca. merci d'avoir sauve ce qui restait de l'ambiance.o

chere mobiliere,

l'autre nuit, je me suis reveille un peu deshabillee, apres avoir accidentellement fait tomber toutes les couvertures du lit. en voulant les recuperer, je me suis pris les pieds dedans et me suis retrouvée par terre, completement enchevêtrée dans les draps. heureusement, vous n'etiez pas là pour voir ca, mais vous avez su m'aider a changer mon matelas après l'incident. merci de me sortir des situations les plus embarrassantes.
";

/// USB

/// Submit a new mouse movement report to the USB stack.
///
/// We do this with interrupts disabled, to avoid a race hazard with the USB IRQ.
fn push_hid_report(report: KeyboardReport) -> Result<usize, usb_device::UsbError> {
    critical_section::with(|_| unsafe {
        // Now interrupts are disabled, grab the global variable and, if
        // available, send it a HID report
        USB_HID.as_mut().map(|hid| hid.push_input(&report))
    })
    .unwrap()
}

/// This function is called whenever the USB Hardware generates an Interrupt
/// Request.
#[allow(non_snake_case)]
#[interrupt]
unsafe fn USBCTRL_IRQ() {
    // Handle USB request
    let usb_dev = USB_DEVICE.as_mut().unwrap();
    let usb_hid = USB_HID.as_mut().unwrap();
    usb_dev.poll(&mut [usb_hid]);
}
