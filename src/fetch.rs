use core::fmt::Write;
use embassy_net::dns::DnsSocket;
use embassy_net::tcp::client::{TcpClient, TcpClientState};
use embassy_net::Stack;
use heapless;
use log::*;
use reqwless::client::HttpClient;
use reqwless::request::Method;
use serde::Deserialize;

use crate::timestamp::Timestamp;

pub async fn fetch_time(stack: &'static Stack<cyw43::NetDriver<'static>>) -> Option<Timestamp> {
    #[derive(Deserialize)]
    struct Response<'a> {
        datetime: &'a str,
    }

    let url = "http://worldtimeapi.org/api/timezone/America/New_York";
    let mut rx_buffer = [0; 1024];

    let json = fetch_json::<Response>(stack, url, &mut rx_buffer).await?;
    info!("Current time: {:?}", json.datetime);
    Timestamp::parse(json.datetime)
}

const VEC_SIZE: usize = 2;

pub async fn fetch_next_bus(
    stack: &'static Stack<cyw43::NetDriver<'static>>,
    route: u8,
    stop: &str,
) -> Option<heapless::Vec<Timestamp, VEC_SIZE>> {
    #[derive(Deserialize)]
    struct Prediction {
        attributes: Attributes,
    }

    #[derive(Deserialize)]
    struct Attributes {
        arrival_time: heapless::String<32>,
    }

    #[derive(Deserialize)]
    struct Response {
        data: heapless::Vec<Prediction, VEC_SIZE>,
    }

    let mut url: heapless::String<100> = heapless::String::new();
    write!(
        &mut url,
        "http://{}/predictions?filter[route]={}&filter[stop]={}&page[limit]={}",
        env!("MBTA_PROXY_IP"),
        route,
        stop,
        VEC_SIZE
    )
    .ok()?;

    let mut rx_buffer = [0; 2048];
    let json = fetch_json::<Response>(stack, url.as_str(), &mut rx_buffer).await?;
    let arrival_times: heapless::Vec<Timestamp, VEC_SIZE> = json
        .data
        .iter()
        .filter_map(|prediction| Timestamp::parse(prediction.attributes.arrival_time.as_str()))
        .collect();

    Some(arrival_times)
}

async fn fetch_json<'a, T>(
    stack: &'static Stack<cyw43::NetDriver<'static>>,
    url: &str,
    rx_buffer: &'a mut [u8],
) -> Option<T>
where
    T: Deserialize<'a>,
{
    let client_state = TcpClientState::<1, 1024, 1024>::new();
    let tcp_client = TcpClient::new(stack, &client_state);
    let dns_client = DnsSocket::new(stack);

    let mut http_client = HttpClient::new(&tcp_client, &dns_client);

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

    let body = match response.body().read_to_end().await {
        Ok(content) => content,
        Err(_e) => {
            error!("Failed to read response body");
            return None;
        }
    };

    match serde_json_core::de::from_slice::<T>(body) {
        Ok((json, _used)) => Some(json),
        Err(_e) => {
            error!("Failed to parse response body");
            None
        }
    }
}
