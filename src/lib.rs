#![no_std]
#![feature(type_alias_impl_trait)]

use cyw43::{Control, NetDriver, PowerManagementMode, Runner, State};
use cyw43_pio::PioSpi;
use embassy_executor::Spawner;
use embassy_net::{Config, Stack, StackResources};
use embassy_rp::clocks::RoscRng;
use embassy_rp::{
    bind_interrupts,
    gpio::{Level, Output},
    peripherals::{DMA_CH1, PIN_23, PIN_24, PIN_25, PIN_29, PIO1, USB},
    pio::{InterruptHandler as PioInterruptHandler, Pio},
    usb::{Driver, InterruptHandler as UsbInterruptHandler},
};
use embassy_time::{Duration, Timer};
use log::*;
use rand::RngCore;
use static_cell::StaticCell;

pub mod fetch;
pub mod rtc;
pub mod timestamp;

pub use fetch::*;
pub use rtc::*;
pub use timestamp::*;

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => UsbInterruptHandler<USB>;
    PIO1_IRQ_0 => PioInterruptHandler<PIO1>;
});

#[embassy_executor::task]
async fn usb_logger_task(driver: Driver<'static, USB>) {
    embassy_usb_logger::run!(1024, log::LevelFilter::Info, driver);
}

#[embassy_executor::task]
async fn net_task(stack: &'static Stack<NetDriver<'static>>) -> ! {
    stack.run().await
}

#[embassy_executor::task]
async fn wifi_task(
    runner: Runner<'static, Output<'static, PIN_23>, PioSpi<'static, PIN_25, PIO1, 0, DMA_CH1>>,
) -> ! {
    runner.run().await
}

pub fn duration_as_minutes(duration: Duration) -> u64 {
    duration.as_secs() / 60
}

pub fn start_usb_logger(spawner: Spawner, usb: USB) {
    let driver = Driver::new(usb, Irqs);
    spawner.spawn(usb_logger_task(driver)).unwrap();
}

pub struct WiFiPins {
    pub pin_23: PIN_23,
    pub pin_24: PIN_24,
    pub pin_25: PIN_25,
    pub pin_29: PIN_29,
    pub pio_1: PIO1,
    pub dma_ch1: DMA_CH1,
}

pub async fn connect_to_wifi(
    spawner: Spawner,
    wifi_ssid: &'static str,
    wifi_password: &'static str,
    pins: WiFiPins,
) -> (&'static Stack<NetDriver<'static>>, Control<'static>) {
    let fw = include_bytes!("../cyw43-firmware/43439A0.bin");
    let clm = include_bytes!("../cyw43-firmware/43439A0_clm.bin");

    // wifi
    let pwr = Output::new(pins.pin_23, Level::Low);
    let cs = Output::new(pins.pin_25, Level::High);
    let mut pio = Pio::new(pins.pio_1, Irqs);
    let spi = PioSpi::new(
        &mut pio.common,
        pio.sm0,
        pio.irq0,
        cs,
        pins.pin_24,
        pins.pin_29,
        pins.dma_ch1,
    );
    static STATE: StaticCell<State> = StaticCell::new();
    let state = STATE.init(State::new());

    let (net_device, mut control, runner) = cyw43::new(state, pwr, spi, fw).await;
    spawner.spawn(wifi_task(runner)).unwrap();

    control.init(clm).await;
    control
        .set_power_management(PowerManagementMode::PowerSave)
        .await;

    let config = Config::dhcpv4(Default::default());

    // Generate random seed
    let mut rng = RoscRng;
    let seed = rng.next_u64();

    // Init network stack
    static STACK: StaticCell<Stack<NetDriver<'static>>> = StaticCell::new();
    static RESOURCES: StaticCell<StackResources<5>> = StaticCell::new();
    let stack = &*STACK.init(Stack::new(
        net_device,
        config,
        RESOURCES.init(StackResources::<5>::new()),
        seed,
    ));

    spawner.spawn(net_task(stack)).unwrap();

    loop {
        match control.join_wpa2(wifi_ssid, wifi_password).await {
            Ok(_) => {
                info!("connected to {}", wifi_ssid);
                break;
            }
            Err(err) => {
                info!("join failed with status={}", err.status);
                Timer::after(Duration::from_secs(10)).await;
            }
        }
    }

    // Wait for DHCP, not necessary when using static IP
    info!("waiting for DHCP...");
    while !stack.is_config_up() {
        Timer::after_millis(100).await;
    }
    info!("DHCP is now up!");

    info!("waiting for link up...");
    while !stack.is_link_up() {
        Timer::after_millis(500).await;
    }
    info!("Link is up!");

    info!("waiting for stack to be up...");
    stack.wait_config_up().await;
    info!("Stack is up!");

    (stack, control)
}
