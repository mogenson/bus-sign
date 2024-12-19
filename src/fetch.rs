use core::str::from_utf8;

use core::fmt::Write;
use embassy_net::dns::DnsSocket;
use embassy_net::tcp::client::{TcpClient, TcpClientState};
use embassy_rp::clocks::RoscRng;
use heapless;
use log::*;
use rand::RngCore;
use reqwless::client::{HttpClient, TlsConfig, TlsVerify};
use reqwless::request::Method;
use serde::Deserialize;

use crate::timestamp::Timestamp;

pub async fn fetch_time(stack: embassy_net::Stack<'_>) -> Option<Timestamp> {
    #[derive(Deserialize)]
    struct Response<'a> {
        datetime: &'a str,
    }

    let mut rx_buffer = [0; 1024];
    let url = "https://worldtimeapi.org/api/timezone/America/New_York";
    let json = fetch_json::<Response>(stack, &mut rx_buffer, url).await?;
    info!("Current time: {:?}", json.datetime);
    Timestamp::parse(json.datetime)
}

pub async fn fetch_next_bus(
    stack: embassy_net::Stack<'_>,
    route: &str,
    stop: &str,
) -> Option<Timestamp> {
    #[derive(Deserialize)]
    struct Attributes<'a> {
        arrival_time: &'a str,
    }

    #[derive(Deserialize)]
    struct Data<'a> {
        #[serde(borrow)]
        attributes: Attributes<'a>,
    }

    #[derive(Deserialize)]
    struct Response<'a> {
        #[serde(borrow)]
        data: heapless::Vec<Data<'a>, 1>,
    }

    let mut rx_buffer = [0; 2048];
    let mut url: heapless::String<100> = heapless::String::new();
    write!(
        &mut url,
        "https://api-v3.mbta.com/predictions?filter[route]={}&filter[stop]={}&page[limit]=1",
        route, stop
    )
    .ok()?;

    let json = fetch_json::<Response>(stack, &mut rx_buffer, url.as_str()).await?;
    info!("Next bus: {:?}", json.data[0].attributes.arrival_time);
    Timestamp::parse(json.data[0].attributes.arrival_time)
}

pub async fn fetch_json<'a, T>(
    stack: embassy_net::Stack<'_>,
    rx_buffer: &'a mut [u8],
    url: &str,
) -> Option<T>
where
    T: Deserialize<'a>,
{
    let mut rng = RoscRng;
    let seed = rng.next_u64();

    let mut tls_read_buffer = [0; 16640];
    let mut tls_write_buffer = [0; 16640];

    let client_state = TcpClientState::<1, 1024, 1024>::new();
    let tcp_client = TcpClient::new(stack, &client_state);
    let dns_client = DnsSocket::new(stack);
    let tls_config = TlsConfig::new(
        seed,
        &mut tls_read_buffer,
        &mut tls_write_buffer,
        TlsVerify::None,
    );

    let mut http_client = HttpClient::new_with_tls(&tcp_client, &dns_client, tls_config);

    info!("connecting to {}", &url);

    let mut request = match http_client.request(Method::GET, url).await {
        Ok(req) => req,
        Err(e) => {
            error!("Failed to make HTTP request: {:?}", e);
            return None;
        }
    };

    let response = match request.send(rx_buffer).await {
        Ok(resp) => resp,
        Err(_e) => {
            error!("Failed to send HTTP request");
            return None;
        }
    };

    let body = match from_utf8(response.body().read_to_end().await.unwrap()) {
        Ok(b) => b,
        Err(_e) => {
            error!("Failed to read response body");
            return None;
        }
    };
    info!("Response body: {:?}", &body);

    let bytes = body.as_bytes();
    match serde_json_core::de::from_slice::<T>(bytes) {
        Ok((output, _used)) => Some(output),
        Err(_e) => {
            error!("Failed to parse response body");
            None
        }
    }
}
