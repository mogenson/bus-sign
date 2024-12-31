#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use bus_sign::fetch::{fetch_next_bus, fetch_time};
use bus_sign::{connect_to_wifi, duration_as_minutes, WiFiPins};
use bus_sign::{rtc, start_usb_logger};
use core::fmt::Write;
use cyw43::NetDriver;
use embassy_executor::Spawner;
use embassy_net::Stack;
use embassy_rp::gpio::{Input, Pull};
use embassy_time::{Duration, Instant, Timer};
use embedded_graphics::mono_font::{ascii::FONT_6X10, MonoTextStyle};
use embedded_graphics::text::Text;
use embedded_graphics::Drawable;
use embedded_graphics_core::{
    pixelcolor::{Rgb888, WebColors},
    prelude::Point,
};
use galactic_unicorn_embassy::pins::{UnicornButtonPins, UnicornDisplayPins, UnicornSensorPins};
use galactic_unicorn_embassy::GalacticUnicorn;
use galactic_unicorn_embassy::{HEIGHT, WIDTH};
use log::*;
use unicorn_graphics::UnicornGraphics;
use {defmt_rtt as _, panic_probe as _};

#[embassy_executor::task(pool_size = 2)]
async fn next_bus_task(
    stack: &'static Stack<NetDriver<'static>>,
    route: u8,
    stop: &'static str,
) -> ! {
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

    start_usb_logger(spawner, p.USB);

    let display_pins = UnicornDisplayPins {
        column_clock: p.PIN_13,
        column_data: p.PIN_14,
        column_latch: p.PIN_15,
        column_blank: p.PIN_16,
        row_bit_0: p.PIN_17,
        row_bit_1: p.PIN_18,
        row_bit_2: p.PIN_19,
        row_bit_3: p.PIN_20,
    };

    let sensor_pins = UnicornSensorPins {
        light_sensor: p.PIN_28,
    };

    let _button_pins = UnicornButtonPins {
        switch_a: Input::new(p.PIN_0, Pull::Up),
        switch_b: Input::new(p.PIN_1, Pull::Up),
        switch_c: Input::new(p.PIN_3, Pull::Up),
        switch_d: Input::new(p.PIN_6, Pull::Up),
        brightness_up: Input::new(p.PIN_21, Pull::Up),
        brightness_down: Input::new(p.PIN_26, Pull::Up),
        volume_up: Input::new(p.PIN_7, Pull::Up),
        volume_down: Input::new(p.PIN_8, Pull::Up),
        sleep: Input::new(p.PIN_27, Pull::Up),
    };

    let mut gu = GalacticUnicorn::new(p.PIO0, display_pins, sensor_pins, p.ADC, p.DMA_CH0);

    let mut graphics = UnicornGraphics::<WIDTH, HEIGHT>::new();
    gu.set_pixels(&graphics);

    // Create a new character style
    let style = MonoTextStyle::new(&FONT_6X10, Rgb888::CSS_PURPLE);
    let mut message = heapless::String::<6>::new();

    let wifi_pins = WiFiPins {
        pin_23: p.PIN_23,
        pin_24: p.PIN_24,
        pin_25: p.PIN_25,
        pin_29: p.PIN_29,
        pio_1: p.PIO1,
        dma_ch1: p.DMA_CH1,
    };
    let (stack, mut control) =
        connect_to_wifi(spawner, env!("WIFI_SSID"), env!("WIFI_PASSWORD"), wifi_pins).await;

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

        let now = rtc::now().await;
        let hour = now.hour;
        let minute = now.minute;

        message.clear();
        write!(&mut message, "{hour}:{minute}").unwrap();

        graphics.fill(Rgb888::new(10, 10, 10));

        Text::new(&message, Point::new(12, 8), style)
            .draw(&mut graphics)
            .unwrap();

        gu.set_pixels(&graphics);
    }
}
