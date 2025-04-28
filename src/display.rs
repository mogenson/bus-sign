use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::mutex::Mutex;
use galactic_unicorn_embassy::{GalacticUnicorn, HEIGHT, WIDTH};
use unicorn_graphics::UnicornGraphics;

pub struct Display<'a> {
    pub gu: GalacticUnicorn<'a>,
    pub graphics: UnicornGraphics<WIDTH, HEIGHT>,
}

pub static DISPLAY: Mutex<ThreadModeRawMutex, Option<Display>> = Mutex::new(None);

pub async fn init(display: Display<'static>) {
    *(DISPLAY.lock().await) = Some(display);
}

pub async fn draw<F>(work_item: F)
where
    F: Fn(&mut Display),
{
    let mut display_locked = DISPLAY.lock().await;
    let display_ref = display_locked.as_mut().unwrap();
    work_item(display_ref);
}
