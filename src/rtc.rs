use embassy_rp::peripherals;
use embassy_rp::rtc;
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::mutex::Mutex;
use log::*;

use crate::timestamp::Timestamp;

static RTC: Mutex<ThreadModeRawMutex, Option<rtc::Rtc<'static, peripherals::RTC>>> =
    Mutex::new(None);

pub async fn init(peripheral: peripherals::RTC, timestamp: Timestamp) {
    let mut rtc = rtc::Rtc::new(peripheral);
    info!("Setting RTC to {:?}", timestamp);
    rtc.set_datetime(timestamp.into()).unwrap();
    *(RTC.lock().await) = Some(rtc);
}

pub async fn now() -> Timestamp {
    let rtc_locked = RTC.lock().await;
    let rtc_ref = rtc_locked.as_ref().unwrap();
    let datetime = rtc_ref.now().unwrap();
    Timestamp::from(datetime)
}
