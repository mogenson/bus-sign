use embassy_rp::clocks::RoscRng;
use embassy_time::{Duration, Ticker};
use galactic_unicorn_embassy::{GalacticUnicorn, HEIGHT, WIDTH};
use log::*;
use rand::RngCore;

const ALIVE: u8 = 100;
const DEAD: u8 = 50;

#[derive(Clone, Copy)]
struct Cell {
    hue: u16,
    light: u8,
}

struct Universe<'a> {
    frame_buffer: [[[Cell; HEIGHT]; WIDTH]; 2],
    display: GalacticUnicorn<'a>,
    page: usize,
    born: usize,
    died: usize,
    stall_count: usize,
}

impl<'a> Universe<'a> {
    pub fn new(gu: GalacticUnicorn<'a>) -> Self {
        Universe {
            frame_buffer: [[[Cell { hue: 0, light: 0 }; HEIGHT]; WIDTH]; 2],
            display: gu,
            page: 0,
            born: 0,
            died: 0,
            stall_count: 0,
        }
    }

    pub fn populate(&mut self) {
        const SEED: usize = WIDTH * HEIGHT * 50 / 100; // 50%
        self.page = 0;
        self.born = 0;
        self.died = 0;
        self.stall_count = 0;
        for x in 0..WIDTH {
            for y in 0..HEIGHT {
                for i in 0..2 {
                    self.frame_buffer[i][x][y] = Cell {
                        hue: (RoscRng.next_u32() % 360) as u16,
                        light: 0,
                    };
                }
            }
        }
        for _ in 0..SEED {
            let x = RoscRng.next_u32() as usize % WIDTH;
            let y = RoscRng.next_u32() as usize % HEIGHT;
            self.frame_buffer[self.page][x][y].light = ALIVE;
        }
    }

    fn next(&mut self, x: usize, y: usize) -> Cell {
        const W: i32 = WIDTH as i32;
        const H: i32 = HEIGHT as i32;
        const NEIGHBORS: [(i32, i32); 8] = [
            (-1, -1),
            (-1, 0),
            (-1, 1),
            (0, 1),
            (0, -1),
            (1, 1),
            (1, -1),
            (1, 0),
        ];
        let mut hues = heapless::Vec::<u16, 8>::new();
        let neighbors: u8 = NEIGHBORS
            .iter()
            .map(|(i, j)| {
                let (col, row) = if let (x @ 0..W, y @ 0..H) = (x as i32 + i, y as i32 + j) {
                    (x as usize, y as usize)
                } else {
                    ((x + WIDTH) % WIDTH, (y + HEIGHT) % HEIGHT)
                };
                let cell = self.frame_buffer[self.page][col][row];
                if cell.light > DEAD {
                    hues.push(cell.hue).unwrap();
                    1
                } else {
                    0
                }
            })
            .sum();

        let mut cell = self.frame_buffer[self.page][x][y];
        match (cell.light > DEAD, neighbors) {
            // rule 1: live cell with less than two live neighbors dies
            (true, x) if x < 2 => {
                self.died += 1;
                cell.light = DEAD;
            }
            // rule 3: live cell with more than 3 live neighbors dies
            (true, x) if x > 3 => {
                self.died += 1;
                cell.light = DEAD;
            }
            // rule 4: dead cell with 3 live neighbors lives
            (false, 3) => {
                self.born += 1;
                cell.hue = hueverage(&hues);
                cell.light = ALIVE;
            }
            // rule 4.1: dead cell with 4 live neighbors lives
            (false, 4) => {
                self.born += 1;
                cell.hue = hueverage(&hues);
                cell.light = ALIVE;
            }
            // dead cell that stays dead had a brightness that slowly decays
            (false, _) => {
                if cell.light > 1 {
                    cell.light -= 2
                };
            }
            // alive cell with 2 or 3 live neightbors stays alive
            (true, _) => { /* no change */ }
        }
        cell
    }

    pub fn step(&mut self) {
        let next_page = (self.page + 1) % 2;
        for x in 0..WIDTH {
            for y in 0..HEIGHT {
                let cell = self.next(x, y);
                self.frame_buffer[next_page][x][y] = cell;
                // info!("cell x {} y {} hue {} light {}", x, y, cell.hue, cell.light);
                let (r, g, b) = hue_to_rgb(cell.hue, cell.light);
                // info!("r {} g {} b {}", r, g, b);
                self.display.set_pixel_rgb(x as u8, y as u8, r, g, b, 255);
            }
        }

        if self.born == self.died {
            self.stall_count += 1;
            info!(
                "born and died: {}, stall count {}",
                self.born, self.stall_count
            );
        } else {
            self.stall_count = 0;
        };

        if self.stall_count >= 5 {
            info!("game stalled, repopulating");
            self.populate();
            return;
        }

        self.born = 0;
        self.died = 0;
        self.page = next_page;
    }
}

fn hueverage(hues: &heapless::Vec<u16, 8>) -> u16 {
    use core::f32::consts::PI;
    let mut x = 0f32;
    let mut y = 0f32;
    for hue in hues.iter() {
        let r = (*hue as f32 / 180.0) * PI;
        x += libm::cosf(r);
        y += libm::sinf(r);
    }
    let extra = ((((RoscRng.next_u32() % 0xFFFF) as f32) - 32767.5) / 65535.0) * PI / 4.0;
    let mut back = libm::atan2f(y, x) + extra;
    if back < 0.0 {
        back += PI * 2.0;
    }
    (back * 180.0 / PI) as u16
}

fn hue_to_rgb(hue: u16, light: u8) -> (u8, u8, u8) {
    let h = hue as f32;
    let s = 1.0;
    let v = light as f32 / 100.0;

    let c = v * s;
    let x = c * (1.0 - libm::fabsf(((h / 60.0) % 2.0) - 1.0));
    let m = v - c;

    let (r, g, b) = match h {
        0.0..60.0 => (c, x, 0.0),
        60.0..120.0 => (x, c, 0.0),
        120.0..180.0 => (0.0, c, x),
        180.0..240.0 => (0.0, x, c),
        240.0..300.0 => (x, 0.0, c),
        300.0..360.0 => (c, 0.0, x),
        _ => (0.0, 0.0, 0.0),
    };

    let r = ((r + m) * 255.0) as u8;
    let g = ((g + m) * 255.0) as u8;
    let b = ((b + m) * 255.0) as u8;

    (r, g, b)
}

pub async fn run(gu: GalacticUnicorn<'_>) {
    let mut ticker = Ticker::every(Duration::from_millis(125));
    let mut universe = Universe::new(gu);
    universe.populate();
    loop {
        ticker.next().await;
        universe.step();
    }
}
