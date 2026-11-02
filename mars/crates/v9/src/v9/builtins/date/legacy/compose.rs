/** Legacy calendar composers validate fields after token collection and apply the documented two-digit-year window. */
use super::*;

/** Parser state retains actual input component order rather than guessing a format from separators. */
#[derive(Default)]
pub(super) struct State {
    pub days: Vec<u32>,
    pub month: Option<u32>,
    pub time: Vec<u32>,
    pub period: Option<u32>,
    pub read_number: bool,
    pub zone_sign: i32,
    pub zone_hours: Option<u32>,
    pub zone_minutes: Option<u32>,
}

impl State {
    /** A zero-offset keyword permits a subsequent signed offset. */
    pub fn utc(&self) -> bool {
        self.zone_hours == Some(0) && self.zone_minutes == Some(0)
    }
    /** A clock consumes only an in-range next component, preventing a later year from becoming seconds. */
    pub fn expects_time(&self, number: u32) -> bool {
        match self.time.len() {
            1 | 2 => number < 60,
            3 => number < 1000,
            _ => false,
        }
    }
    /** Time capacity is fixed at hour, minute, second and millisecond. */
    pub fn add_time(&mut self, number: u32) -> Option<()> {
        if self.time.len() >= 4 {
            return None;
        }
        self.time.push(number);
        Some(())
    }
    /** A completed clock defaults all remaining components to zero. */
    pub fn final_time(&mut self, number: u32) -> Option<()> {
        self.add_time(number)?;
        self.time.resize(4, 0);
        Some(())
    }
    /** Final assembly validates calendar and clock ranges before normalizing civil day overflow. */
    pub fn finish(mut self) -> Option<(f64, bool)> {
        if self.days.is_empty() {
            return None;
        }
        self.days.resize(3, 1);
        let (year, month, day) = match self.month {
            Some(month) if !(1..=31).contains(&self.days[0]) => (self.days[0], month, self.days[1]),
            Some(month) => (self.days[1], month, self.days[0]),
            None if !(1..=31).contains(&self.days[0]) => (self.days[0], self.days[1], self.days[2]),
            None => (self.days[2], self.days[0], self.days[1]),
        };
        if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
            return None;
        }
        let year = if year < 50 {
            year + 2000
        } else if year < 100 {
            year + 1900
        } else {
            year
        };
        self.time.resize(4, 0);
        if let Some(period) = self.period {
            if self.time[0] > 12 {
                return None;
            }
            self.time[0] = self.time[0] % 12 + period;
        }
        let clock = &self.time;
        if clock[0] > 24
            || clock[1] > 59
            || clock[2] > 59
            || clock[3] > 999
            || clock[0] == 24 && clock[1..].iter().any(|value| *value != 0)
        {
            return None;
        }
        let offset = self
            .zone_hours
            .unwrap_or(0)
            .checked_mul(3600)?
            .checked_add(self.zone_minutes.unwrap_or(0).checked_mul(60)?)?;
        if offset > i32::MAX as u32 {
            return None;
        }
        let value = make_date(
            f64::from(year),
            f64::from(month - 1),
            f64::from(day),
            f64::from(clock[0]),
            f64::from(clock[1]),
            f64::from(clock[2]),
            f64::from(clock[3]),
        ) - f64::from(self.zone_sign) * f64::from(offset) * SECOND;
        Some((value, self.zone_sign != 0))
    }
}
