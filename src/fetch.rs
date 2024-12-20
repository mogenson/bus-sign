use core::str::from_utf8;

use core::fmt::Write;
use embassy_net::dns::DnsSocket;
use embassy_net::tcp::client::{TcpClient, TcpClientState};
use heapless;
use log::*;
use reqwless::client::HttpClient;
use reqwless::request::Method;
use serde::Deserialize;

use crate::timestamp::Timestamp;

#[derive(Deserialize, Debug)]
struct Response<'a> {
    datetime: &'a str,
}

pub async fn fetch_time(stack: embassy_net::Stack<'_>) -> Option<Timestamp> {
    let url = "http://worldtimeapi.org/api/timezone/America/New_York";
    http_get_timestamp(stack, url).await
}

pub async fn fetch_next_bus(
    stack: embassy_net::Stack<'_>,
    route: u8,
    stop: &str,
) -> Option<Timestamp> {
    let mut url: heapless::String<100> = heapless::String::new();
    write!(
        &mut url,
        "http://{}/predictions?filter[route]={}&filter[stop]={}&page[limit]=1",
        env!("MBTA_PROXY_IP"),
        route,
        stop
    )
    .ok()?;
    http_get_timestamp(stack, url.as_str()).await
}

async fn http_get_timestamp(
    stack: embassy_net::Stack<'_>,
    url: &str,
) -> Option<Timestamp> {
    let client_state = TcpClientState::<1, 1024, 1024>::new();
    let tcp_client = TcpClient::new(stack, &client_state);
    let dns_client = DnsSocket::new(stack);
    let mut rx_buffer = [0; 1024];

    let mut http_client = HttpClient::new(&tcp_client, &dns_client);

    info!("connecting to {}", &url);

    let mut request = match http_client.request(Method::GET, url).await {
        Ok(req) => req,
        Err(e) => {
            error!("Failed to make HTTP request: {:?}", e);
            return None;
        }
    };

    let response = match request.send(&mut rx_buffer).await {
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
    match serde_json_core::de::from_slice::<Response>(bytes) {
        Ok((json, _used)) => Timestamp::parse(json.datetime),
        Err(_e) => {
            error!("Failed to parse response body");
            None
        }
    }
}
