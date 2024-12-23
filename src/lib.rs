#![no_std]

use embassy_executor::Spawner;
use embassy_net::{Config, Stack, StackResources};
use embassy_rp::clocks::RoscRng;
use embassy_time::{Duration, Timer};
use log::*;
use rand::RngCore;
use static_cell::StaticCell;

use cyw43::{Control, NetDriver};

pub mod fetch;
pub mod rtc;
pub mod timestamp;

pub use fetch::*;
pub use rtc::*;
pub use timestamp::*;

#[embassy_executor::task]
async fn net_task(mut runner: embassy_net::Runner<'static, cyw43::NetDriver<'static>>) -> ! {
    runner.run().await
}

pub fn duration_as_minutes(duration: Duration) -> u64 {
    duration.as_secs() / 60
}

pub async fn connect_to_wifi(
    spawner: Spawner,
    net_device: NetDriver<'static>,
    control: &mut Control<'static>,
    wifi_ssid: &'static str,
    wifi_password: &'static str,
) -> Stack<'static> {
    let config = Config::dhcpv4(Default::default());

    // Generate random seed
    let mut rng = RoscRng;
    let seed = rng.next_u64();

    // Init network stack
    static RESOURCES: StaticCell<StackResources<5>> = StaticCell::new();
    let (stack, runner) = embassy_net::new(
        net_device,
        config,
        RESOURCES.init(StackResources::new()),
        seed,
    );

    spawner.spawn(net_task(runner)).unwrap();

    loop {
        match control.join_wpa2(wifi_ssid, wifi_password).await {
            Ok(_) => {
                info!("connected to {}", wifi_ssid);
                break;
            }
            Err(err) => {
                info!("join failed with status={}", err.status);
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

    stack
}
