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
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::{Duration, Instant, Timer};
use embedded_graphics::mono_font::{ascii::FONT_4X6, MonoTextStyle};
use embedded_graphics::pixelcolor::Rgb888;
use embedded_graphics::prelude::{Point, Primitive, RgbColor, Size};
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
use embedded_graphics::text::Text;
use embedded_graphics::Drawable;
use galactic_unicorn_embassy::pins::{UnicornButtonPins, UnicornDisplayPins, UnicornSensorPins};
use galactic_unicorn_embassy::GalacticUnicorn;
use galactic_unicorn_embassy::{HEIGHT, WIDTH};
use log::*;
use unicorn_graphics::UnicornGraphics;
use {defmt_rtt as _, panic_probe as _};

use embedded_graphics::prelude::WebColors;

#[derive(Copy, Clone)]
enum Route {
    EightySeven,
    EightyEight,
}

impl From<Route> for u8 {
    fn from(val: Route) -> Self {
        match val {
            Route::EightySeven => 87,
            Route::EightyEight => 88,
        }
    }
}

enum DisplayCommand {
    Off,
    On,
    Message(DisplayMessage),
}

struct DisplayMessage {
    pub route: Route,
    pub value: u8,
}

static CHANNEL: Channel<ThreadModeRawMutex, DisplayCommand, 8> = Channel::new();

#[embassy_executor::task]
async fn display_task(
    mut gu: GalacticUnicorn<'static>,
    mut graphics: UnicornGraphics<WIDTH, HEIGHT>,
) -> ! {
    let mut string = heapless::String::<16>::new();
    let yellow = MonoTextStyle::new(&FONT_4X6, Rgb888::CSS_GOLD);
    let orange = MonoTextStyle::new(&FONT_4X6, Rgb888::CSS_ORANGE);
    let white = MonoTextStyle::new(&FONT_4X6, Rgb888::WHITE);
    let black = PrimitiveStyle::with_fill(Rgb888::BLACK);

    fn draw_label(
        route: &str,
        baseline: i32,
        color: MonoTextStyle<Rgb888>,
        graphics: &mut UnicornGraphics<WIDTH, HEIGHT>,
    ) {
        Text::new(route, Point::new(0, baseline), color)
            .draw(graphics)
            .unwrap();
        Text::new("BUS", Point::new(9, baseline), color)
            .draw(graphics)
            .unwrap();
        Text::new("IN", Point::new(22, baseline), color)
            .draw(graphics)
            .unwrap();
        Text::new("MIN", Point::new(42, baseline), color)
            .draw(graphics)
            .unwrap();
    }

    draw_label("87", 4, yellow, &mut graphics);
    draw_label("88", 10, orange, &mut graphics);
    gu.set_pixels(&graphics);

    loop {
        match CHANNEL.receive().await {
            DisplayCommand::Off => {
                gu.brightness = 0;
                gu.set_pixels(&graphics);
            }
            DisplayCommand::On => {
                gu.brightness = 100;
                gu.set_pixels(&graphics);
            }
            DisplayCommand::Message(display_message) => {
                let value = display_message.value;
                let x = if value > 9 { 32 } else { 36 };
                string.clear();
                write!(&mut string, "{value}").unwrap();

                match display_message.route {
                    Route::EightySeven => {
                        Rectangle::new(Point::new(31, 0), Size::new(9, 5))
                            .into_styled(black)
                            .draw(&mut graphics)
                            .unwrap();

                        Text::new(&string, Point::new(x, 4), white)
                            .draw(&mut graphics)
                            .unwrap();
                        gu.set_pixels(&graphics);
                    }
                    Route::EightyEight => {
                        Rectangle::new(Point::new(31, 6), Size::new(9, 5))
                            .into_styled(black)
                            .draw(&mut graphics)
                            .unwrap();

                        Text::new(&string, Point::new(x, 10), white)
                            .draw(&mut graphics)
                            .unwrap();
                        gu.set_pixels(&graphics);
                    }
                }
            }
        }
    }
}

#[embassy_executor::task(pool_size = 2)]
async fn next_bus_task(
    stack: &'static Stack<NetDriver<'static>>,
    route: Route,
    stop: &'static str,
) -> ! {
    let one_minute = Duration::from_secs(60);
    let channel = CHANNEL.sender();
    loop {
        let route_u8 = u8::from(route);
        let Some(arrival_time) = fetch_next_bus(stack, route_u8, stop).await else {
            Timer::after(one_minute).await;
            continue;
        };
        info!(
            "Route {}: next bus arrives at: {:?}",
            route_u8, arrival_time
        );

        let next_bus_time = Instant::from(arrival_time);
        let now = Instant::from(rtc::now().await);
        let delta = next_bus_time.saturating_duration_since(now);
        let wait_time = core::cmp::max(delta / 2, one_minute);
        let next_fetch_time = now + wait_time;
        info!(
            "Route {}: waiting {} min to fetch again",
            route_u8,
            duration_as_minutes(wait_time)
        );

        loop {
            let current_time = rtc::now().await;

            // sleep for 12 hours after 7 pm with display off
            if current_time.hour >= 19 {
                info!("Turning display off");
                channel.send(DisplayCommand::Off).await;

                info!("Sleeping for 12 hours");
                Timer::after_secs(12 * 60 * 60).await;

                info!("Turning display on");
                channel.send(DisplayCommand::On).await;
                break;
            }

            let now = Instant::from(current_time);
            if now > next_fetch_time {
                break;
            }

            let delta = next_bus_time.saturating_duration_since(now);
            let value = duration_as_minutes(delta) as u8;

            info!("Route {}: time to next bus: {} min", route_u8, value);

            channel
                .send(DisplayCommand::Message(DisplayMessage { route, value }))
                .await;

            Timer::after_secs(10).await;
        }
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
    gu.brightness = 100;
    let graphics = UnicornGraphics::<WIDTH, HEIGHT>::new();
    gu.set_pixels(&graphics);

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

    spawner.spawn(display_task(gu, graphics)).unwrap();

    spawner
        .spawn(next_bus_task(stack, Route::EightySeven, env!("BUS_STOP")))
        .unwrap();

    spawner
        .spawn(next_bus_task(stack, Route::EightyEight, env!("BUS_STOP")))
        .unwrap();

    loop {
        control.gpio_set(0, true).await;
        Timer::after_secs(1).await;
        control.gpio_set(0, false).await;
        Timer::after_secs(1).await;
    }
}
