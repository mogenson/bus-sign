#![no_std]

/// Same as a DateTime without the day_of_week member
#[derive(core::fmt::Debug, defmt::Format)]
pub struct Timestamp {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

impl Timestamp {
    fn new(year: u16, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> Self {
        Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
        }
    }

    /// parse iso8601 string
    pub fn parse(value: &str) -> Self {
        // ex: "2024-12-15T14:40:18.167264-05:00"
        let year = value[0..4].parse::<u16>().unwrap();
        let month = value[5..7].parse::<u8>().unwrap();
        let day = value[8..10].parse::<u8>().unwrap();
        let hour = value[11..13].parse::<u8>().unwrap();
        let minute = value[14..16].parse::<u8>().unwrap();
        let second = value[17..19].parse::<u8>().unwrap();
        Timestamp::new(year, month, day, hour, minute, second)
    }
}
