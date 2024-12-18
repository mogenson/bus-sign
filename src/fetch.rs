use core::str::from_utf8;

use embassy_net::dns::DnsSocket;
use embassy_net::tcp::client::{TcpClient, TcpClientState};
use log::*;
use reqwless::client::HttpClient;
use reqwless::request::Method;
use serde::Deserialize;

use crate::timestamp::Timestamp;

pub async fn fetch_time(stack: embassy_net::Stack<'_>) -> Option<Timestamp> {
    #[derive(Deserialize)]
    struct WorldTimeApiResponse<'a> {
        datetime: &'a str,
    }

    let mut rx_buffer = [0; 1024];
    let url = "http://worldtimeapi.org/api/timezone/America/New_York";
    let json = fetch_json::<WorldTimeApiResponse>(stack, &mut rx_buffer, url).await?;
    info!("Current time: {:?}", json.datetime);
    Timestamp::parse(json.datetime)
}

pub async fn fetch_json<'a, T>(
    stack: embassy_net::Stack<'_>,
    rx_buffer: &'a mut [u8],
    url: &str,
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
