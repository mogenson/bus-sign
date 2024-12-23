#![no_std]
#![no_main]
#![allow(async_fn_in_trait)]

use bus_sign::fetch::{fetch_next_bus, fetch_time};
use bus_sign::rtc;
use bus_sign::{connect_to_wifi, duration_as_minutes};
use cyw43_pio::PioSpi;
use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::peripherals::{DMA_CH0, PIO0, USB};
use embassy_rp::pio::{InterruptHandler as PioInterruptHandler, Pio};
use embassy_rp::usb::{Driver, InterruptHandler as UsbInterruptHandler};
use embassy_time::{Duration, Instant, Timer};
use log::*;
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => UsbInterruptHandler<USB>;
    PIO0_IRQ_0 => PioInterruptHandler<PIO0>;
});

#[embassy_executor::task]
async fn logger_task(driver: Driver<'static, USB>) {
    embassy_usb_logger::run!(1024, log::LevelFilter::Info, driver);
}

#[embassy_executor::task]
async fn cyw43_task(
    runner: cyw43::Runner<'static, Output<'static>, PioSpi<'static, PIO0, 0, DMA_CH0>>,
) -> ! {
    runner.run().await
}

#[embassy_executor::task(pool_size = 2)]
async fn next_bus_task(stack: embassy_net::Stack<'static>, route: u8, stop: &'static str) {
    let one_minute = Duration::from_secs(60);
    loop {
        let Some(arrival_time) = fetch_next_bus(stack, route, stop).await else {
            Timer::after(one_minute).await;
            continue;
        };
        info!("Route {}: next bus arrives at: {:?}", route, arrival_time);

        let next_bus = Instant::from(arrival_time);
        let now = Instant::from(rtc::now().await);
        let delta = next_bus.saturating_duration_since(now);

        info!(
            "Route {}: time to next bus: {} min",
            route,
            duration_as_minutes(delta)
        );

        let wait = core::cmp::max(delta / 2, one_minute);
        info!(
            "Route {}: waiting {} min to fetch again",
            route,
            duration_as_minutes(wait)
        );
        Timer::after(wait).await;
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let driver = Driver::new(p.USB, Irqs);
    spawner.spawn(logger_task(driver)).unwrap();

    let fw = include_bytes!("../cyw43-firmware/43439A0.bin");
    let clm = include_bytes!("../cyw43-firmware/43439A0_clm.bin");

    let pwr = Output::new(p.PIN_23, Level::Low);
    let cs = Output::new(p.PIN_25, Level::High);
    let mut pio = Pio::new(p.PIO0, Irqs);
    let spi = PioSpi::new(
        &mut pio.common,
        pio.sm0,
        pio.irq0,
        cs,
        p.PIN_24,
        p.PIN_29,
        p.DMA_CH0,
    );

    static STATE: StaticCell<cyw43::State> = StaticCell::new();
    let state = STATE.init(cyw43::State::new());
    let (net_device, mut control, runner) = cyw43::new(state, pwr, spi, fw).await;
    spawner.spawn(cyw43_task(runner)).unwrap();

    control.init(clm).await;
    control
        .set_power_management(cyw43::PowerManagementMode::PowerSave)
        .await;

    let stack = connect_to_wifi(
        spawner,
        net_device,
        &mut control,
        env!("WIFI_SSID"),
        env!("WIFI_PASSWORD"),
    )
    .await;

    let mut wait = Duration::from_secs(2);
    let now = loop {
        if let Some(now) = fetch_time(stack).await {
            break now;
        }
        Timer::after(wait).await;
        wait *= 2;
    };

    rtc::init(p.RTC, now).await;

    spawner
        .spawn(next_bus_task(stack, 87, env!("BUS_STOP")))
        .unwrap();

    spawner
        .spawn(next_bus_task(stack, 88, env!("BUS_STOP")))
        .unwrap();

    loop {
        control.gpio_set(0, true).await;
        Timer::after_secs(1).await;
        control.gpio_set(0, false).await;
        Timer::after_secs(1).await;
    }
}
