#![no_std]

use embassy_rp::rtc::{DateTime, DayOfWeek};

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
        let mut chars = value.chars();

        if value.len() < 20 {
            return None;
        }

        let year: u16 = value[0..4].parse().ok()?;

        if chars.nth(4)? != '-' {
            return None;
        }

        let month: u8 = value[5..7].parse().ok()?;

        if chars.nth(7)? != '-' {
            return None;
        }

        let day: u8 = value[8..10].parse().ok()?;

        if chars.nth(10)? != 'T' {
            return None;
        }

        let hour: u8 = value[11..13].parse().ok()?;

        if chars.nth(13)? != ':' {
            return None;
        }

        let minute: u8 = value[14..16].parse().ok()?;

        if chars.nth(16)? != ':' {
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

impl Into<DateTime> for Timestamp {
    fn into(self) -> DateTime {
        DateTime {
            year: self.year,
            month: self.month,
            day: self.day,
            day_of_week: DayOfWeek::Sunday, // dummy value
            hour: self.hour,
            minute: self.minute,
            second: self.second,
        }
    }
}
