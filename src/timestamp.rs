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

    fn seconds_from_midnight(self) -> u64 {
        self.hour as u64 * 3600 + self.minute as u64 * 60 + self.second as u64
    }

    /// returns seconds between self and future if future is in the future, else None
    pub fn seconds_until(self, future: Self) -> Option<u64> {
        let now = self.seconds_from_midnight();
        let then = future.seconds_from_midnight();
        if then > now {
            Some(then - now)
        } else {
            None
        }
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
