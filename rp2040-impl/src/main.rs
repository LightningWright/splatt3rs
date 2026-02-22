#![no_std]
#![no_main]

use bsp::{entry, hal};
use cortex_m::prelude::*;
use defmt::*;
use defmt_rtt as _;
use embedded_hal::digital::*;
use fugit::ExtU32;
use hal::pac;
use panic_probe as _;
use rp_pico as bsp;
use usb_device::{class_prelude::*, device::UsbRev, prelude::*};
use usbd_human_interface_device::{
    descriptor::InterfaceProtocol,
    interface::{InBytes8, InterfaceBuilder, OutBytes8, Reports64},
    prelude::*,
};

pub mod interpreter;
use interpreter::{InstrInterpreter, parse_instructions};

mod switch_hid;
use switch_hid::*;

const USB_POLL_DELAY: u32 = 1;
const START_DELAY: u32 = 5000;
// 80ms 50% cycle -> 40ms pressed, 40ms released
const INSTR_DELAY: u32 = 80; // Change this for faster input cycle. 80ms is safe
const RELEASE_DELAY: u32 = 40; // Change this for faster button release. 40ms is safe

#[entry]
fn main() -> ! {
    // Setup Peripherals, Clocks, Timers, and I/O
    let mut pac = pac::Peripherals::take().unwrap();
    let mut watchdog = hal::Watchdog::new(pac.WATCHDOG);
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

    let timer = hal::Timer::new(pac.TIMER, &mut pac.RESETS, &clocks);

    let sio = hal::Sio::new(pac.SIO);
    let pins = hal::gpio::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    // Setup RP-PICO as a valid USB HID device
    let usb_bus = hal::usb::UsbBus::new(
        pac.USBCTRL_REGS,
        pac.USBCTRL_DPRAM,
        clocks.usb_clock,
        true,
        &mut pac.RESETS,
    );

    let usb_bus_alloc = UsbBusAllocator::new(usb_bus);

    let gamepad_if =
        InterfaceBuilder::<InBytes8, OutBytes8, Reports64>::new(SWITCH_REPORT_DESCRIPTOR)
            .unwrap()
            .in_endpoint(1.millis())
            .unwrap()
            .with_out_endpoint(1.millis())
            .unwrap()
            .boot_device(InterfaceProtocol::None)
            .build();

    let mut gamepad = UsbHidClassBuilder::new()
        .add_device(gamepad_if)
        .build(&usb_bus_alloc);

    let mut usb_dev = UsbDeviceBuilder::new(&usb_bus_alloc, UsbVidPid(0x0F0D, 0x0092))
        .device_class(0) // Use class information in the Interface Descriptors
        .device_sub_class(0)
        .device_protocol(0)
        .max_packet_size_0(64)
        .unwrap()
        .usb_rev(UsbRev::Usb200)
        .device_release(0x0100)
        .strings(&[StringDescriptors::new(LangID::EN_US)
            .manufacturer("HORI CO.,LTD.")
            .product("POKKEN CONTROLLER")])
        .unwrap()
        .self_powered(false)
        .supports_remote_wakeup(false)
        .max_power(100) // 500mA
        .unwrap()
        .build();

    info!("[RP2040] Initialized");

    // Use the on-board led for signaling button presses.
    let mut led_pin = pins.gpio25.into_push_pull_output();

    // Load instructions file. It uses RLE compression
    let instrs_rle = include_bytes!("../../instrs.rle");
    let instrs = parse_instructions(instrs_rle).unwrap();
    let mut interpreter = InstrInterpreter::new(&instrs);

    // Struct that holds the report the controller sends to the Nintendo Switch
    let mut report = SwitchReport::new();

    // Initialize timers
    let mut usb_poll_timer = timer.count_down();
    let mut start_timer = timer.count_down();
    let mut instruction_timer = timer.count_down();
    let mut release_timer = timer.count_down();
    let mut force_release = false;

    usb_poll_timer.start(USB_POLL_DELAY.millis());
    start_timer.start(START_DELAY.millis());
    instruction_timer.start(INSTR_DELAY.millis());
    release_timer.start(RELEASE_DELAY.millis());

    let mut usb_wouldblock = 0; // Just for debugging purposes

    // Wait some time while sending null reports so the Switch triggers controller selection menu
    loop {
        if start_timer.wait().is_ok() {
            break;
        }
        usb_dev.poll(&mut [&mut gamepad]);
    }

    let mut rx_report = [0u8; 256];
    let mut tx_report_id = 0;
    let mut tx_report_ack_id = 0;

    // Main loop. Advances the interpreter state machine and sends a new report
    loop {
        if usb_poll_timer.wait().is_ok() {
            // Check that previous report has been acknowledged. If so, perform a new step
            if tx_report_id == tx_report_ack_id && instruction_timer.wait().is_ok() {
                release_timer.start(RELEASE_DELAY.millis());
                force_release = true;

                if let Some(r) = interpreter.step() {
                    tx_report_id += 1;
                    report = r;
                    led_pin.set_high();
                    // let hat: SwitchHat = unsafe { core::mem::transmute(report.hat) };
                    // debug!(" Press {} {:?}", report.buttons, hat);
                }
            }

            if force_release && release_timer.wait().is_ok() {
                force_release = false;
                led_pin.set_low();
                report = SwitchReport::new();
            }

            let bytes: &[u8] = bytemuck::bytes_of(&report);

            usb_dev.poll(&mut [&mut gamepad]);
            match gamepad.device().write_report(bytes) {
                Ok(_) => {
                    // Report ack'ed
                    tx_report_ack_id = tx_report_id;

                    if usb_wouldblock > 0 {
                        // warn!("USB Busy {}", usb_wouldblock);
                        usb_wouldblock = 0;
                    }
                }
                Err(UsbError::WouldBlock) => {
                    // We are saturating the bus, wait...
                    // Ideally we should poll only at a rate of 8ms. Sadly, it performs worse.
                    usb_wouldblock += 1;
                }
                Err(e) => {
                    error!("{}", e);
                }
            }

            // Always accept reports sent by the Switch
            if let Ok(rx_sz) = gamepad.device().read_report(&mut rx_report) {
                for b in &rx_report[0..rx_sz] {
                    debug!("{}", b)
                }
            }
        }
        usb_dev.poll(&mut [&mut gamepad]);
    }
}
