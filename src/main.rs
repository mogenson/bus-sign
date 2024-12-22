#![no_std]
#![no_main]
#![allow(async_fn_in_trait)]

use bus_sign::connect_to_wifi;
use bus_sign::fetch::{fetch_next_bus, fetch_time};
use bus_sign::timestamp::Timestamp;
use cyw43_pio::PioSpi;
use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::peripherals::{DMA_CH0, PIO0, RTC, USB};
use embassy_rp::pio::{InterruptHandler as PioInterruptHandler, Pio};
use embassy_rp::rtc::Rtc;
use embassy_rp::usb::{Driver, InterruptHandler as UsbInterruptHandler};
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Timer};
use log::*;
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

type RtcType<'a> = Mutex<ThreadModeRawMutex, Option<Rtc<'a, RTC>>>;
static SHARED_RTC: RtcType = Mutex::new(None);

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
async fn next_bus_task(
    stack: embassy_net::Stack<'static>,
    rtc: &'static RtcType<'static>,
    route: u8,
    stop: &'static str,
) {
    let one_minute = 60;
    loop {
        let Some(next_bus) = fetch_next_bus(stack, route, stop).await else {
            Timer::after_secs(one_minute).await;
            continue;
        };

        info!("bus route {} time {:?}", route, next_bus);

        let rtc_unlocked = rtc.lock().await;
        let Some(rtc_ref) = rtc_unlocked.as_ref() else {
            Timer::after_secs(one_minute).await;
            continue;
        };

        let Ok(datetime) = rtc_ref.now() else {
            Timer::after_secs(one_minute).await;
            continue;
        };

        // let Ok(datetime) = rtc.now() else {
        //     Timer::after_secs(one_minute).await;
        //     continue;
        // };

        let now = Timestamp::from(datetime);
        let Some(delta) = now.seconds_until(next_bus) else {
            Timer::after_secs(one_minute).await;
            continue;
        };

        let minutes = delta / 2;
        info!("{:?} minutes until next bus", minutes);
        let wait = core::cmp::min(minutes, one_minute);
        Timer::after_secs(wait).await;
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let mut rtc = Rtc::new(p.RTC);

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

    let mut wait: u64 = 2;
    loop {
        if let Some(now) = fetch_time(stack).await {
            info!("Setting RTC to {:?}", now);
            rtc.set_datetime(now.into()).unwrap();
            break;
        }
        Timer::after(Duration::from_secs(wait)).await;
        wait *= 2;
    }

    {
        *(SHARED_RTC.lock().await) = Some(rtc);
    }

    spawner
        .spawn(next_bus_task(stack, &SHARED_RTC, 87, env!("BUS_STOP")))
        .unwrap();

    spawner
        .spawn(next_bus_task(stack, &SHARED_RTC, 88, env!("BUS_STOP")))
        .unwrap();

    loop {
        control.gpio_set(0, true).await;
        Timer::after_secs(1).await;
        control.gpio_set(0, false).await;
        Timer::after_secs(1).await;
    }
}
