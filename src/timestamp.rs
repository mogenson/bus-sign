use embassy_rp::rtc::{DateTime, DayOfWeek};
use embassy_time::Instant;

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
    /// parse iso8601 string
    pub fn parse(value: &str) -> Option<Self> {
        // ex: "2024-12-15T14:40:18.167264-05:00"
        if value.len() < 20 {
            return None;
        }
        let year: u16 = value[0..4].parse().ok()?;
        if value[4..].chars().next()? != '-' {
            return None;
        }

        let month: u8 = value[5..7].parse().ok()?;

        if value[7..].chars().next()? != '-' {
            return None;
        }

        let day: u8 = value[8..10].parse().ok()?;

        if value[10..].chars().next()? != 'T' {
            return None;
        }

        let hour: u8 = value[11..13].parse().ok()?;

        if value[13..].chars().next()? != ':' {
            return None;
        }

        let minute: u8 = value[14..16].parse().ok()?;

        if value[16..].chars().next()? != ':' {
            return None;
        }

        let second: u8 = value[17..19].parse().ok()?;

        Some(Timestamp {
            year,
            month,
            day,
            hour,
            minute,
            second,
        })
    }

    fn as_secs(&self) -> u64 {
        self.year as u64 * 31536000
            + (match self.month as u64 {
                1 => 31,
                2 => 31 + 28,
                3 => 31 + 28 + 31,
                4 => 31 + 28 + 31 + 30,
                5 => 31 + 28 + 31 + 30 + 31,
                6 => 31 + 28 + 31 + 30 + 31 + 30,
                7 => 31 + 28 + 31 + 30 + 31 + 30 + 31,
                8 => 31 + 28 + 31 + 30 + 31 + 30 + 31 + 31,
                9 => 31 + 28 + 31 + 30 + 31 + 30 + 31 + 31 + 30,
                10 => 31 + 28 + 31 + 30 + 31 + 30 + 31 + 31 + 30 + 31,
                11 => 31 + 28 + 31 + 30 + 31 + 30 + 31 + 31 + 30 + 31 + 30,
                12 => 31 + 28 + 31 + 30 + 31 + 30 + 31 + 31 + 30 + 31 + 30 + 31,
                _ => 0,
            }) * 86400
            + self.day as u64 * 86400
            + self.hour as u64 * 3600
            + self.minute as u64 * 60
            + self.second as u64
    }
}

impl From<DateTime> for Timestamp {
    fn from(value: DateTime) -> Self {
        Timestamp {
            year: value.year,
            month: value.month,
            day: value.day,
            hour: value.hour,
            minute: value.minute,
            second: value.second,
        }
    }
}

impl From<Timestamp> for DateTime {
    fn from(val: Timestamp) -> Self {
        DateTime {
            year: val.year,
            month: val.month,
            day: val.day,
            day_of_week: DayOfWeek::Sunday, // dummy value
            hour: val.hour,
            minute: val.minute,
            second: val.second,
        }
    }
}

impl From<Timestamp> for Instant {
    fn from(val: Timestamp) -> Self {
        Instant::from_secs(val.as_secs())
    }
}
