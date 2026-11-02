/** POSIX TZ recurrence computes future DST transitions from Gregorian rules and explicit time bases. */
use super::*;
mod parse;

/** A recurrence day is either a month/week/weekday or a Julian/ordinal day in its year. */
#[derive(Clone, Copy)]
pub(super) enum Day {
    Month(u32, u32, u32),
    Julian(u32),
    Ordinal(u32),
}
/** Transition time may be local wall time, standard time, or UTC as allowed by TZif footers. */
#[derive(Clone, Copy)]
pub(super) enum Basis {
    Wall,
    Standard,
    Utc,
}
/** A rule's time can lie outside its nominal day, including negative and extended hours. */
#[derive(Clone, Copy)]
pub(super) struct Transition {
    pub day: Day,
    pub seconds: i32,
    pub basis: Basis,
}
/** Parsed offsets are seconds east of UTC, opposite POSIX's textual sign convention. */
pub(super) struct Rule {
    pub standard: i32,
    pub daylight: i32,
    pub start: Option<Transition>,
    pub end: Option<Transition>,
}

impl Rule {
    /** Parsing must consume every byte so a malformed host footer never silently selects a partial rule. */
    pub fn parse(text: &str) -> Option<Self> {
        parse::parse(text)
    }
    /** The most recent neighboring-year transition determines the current offset, including southern summers. */
    pub fn offset(&self, time: f64) -> i32 {
        let year = parts(time).year as i64;
        (year - 1..=year + 1)
            .flat_map(|year| self.events(year))
            .filter(|event| event.0 <= time)
            .max_by(|left, right| left.0.total_cmp(&right.0))
            .map_or(self.standard, |event| event.2)
    }
    /** A daylight name follows the last start/end event, including zero-width or negative daylight shifts. */
    pub fn is_daylight(&self, time: f64) -> bool {
        let year = parts(time).year as i64;
        (year - 1..=year + 1)
            .flat_map(|year| self.events(year).into_iter().enumerate())
            .filter(|(_, event)| event.0 <= time)
            .max_by(|(_, left), (_, right)| left.0.total_cmp(&right.0))
            .is_some_and(|(index, _)| index == 0)
    }
    /** Every event carries its before/after offsets so local-time gaps use the actual transition displacement. */
    pub fn events(&self, year: i64) -> Vec<(f64, i32, i32)> {
        match (self.start, self.end) {
            (Some(start), Some(end)) => vec![
                (
                    instant(start, year, self.standard, self.standard),
                    self.standard,
                    self.daylight,
                ),
                (
                    instant(end, year, self.daylight, self.standard),
                    self.daylight,
                    self.standard,
                ),
            ],
            _ => Vec::new(),
        }
    }
}

/** Civil rules normalize week five to the final matching weekday of the requested month. */
fn instant(transition: Transition, year: i64, before: i32, standard: i32) -> f64 {
    let day = match transition.day {
        Day::Month(month, week, weekday) => {
            let first = days_from_civil(year as f64, f64::from(month - 1), 1.0);
            let first_weekday = (first + 4.0).rem_euclid(7.0) as u32;
            let mut day = 1 + (weekday + 7 - first_weekday) % 7 + 7 * (week - 1);
            let days = (days_from_civil(year as f64, f64::from(month), 1.0) - first) as u32;
            if day > days {
                day -= 7;
            }
            first + f64::from(day - 1)
        }
        Day::Julian(day) => {
            days_from_civil(year as f64, 0.0, 1.0)
                + f64::from(day - 1)
                + if leap(year) && day >= 60 { 1.0 } else { 0.0 }
        }
        Day::Ordinal(day) => days_from_civil(year as f64, 0.0, 1.0) + f64::from(day),
    };
    let offset = match transition.basis {
        Basis::Wall => before,
        Basis::Standard => standard,
        Basis::Utc => 0,
    };
    day * DAY + f64::from(transition.seconds - offset) * SECOND
}
/** Proleptic Gregorian leap rules are valid across Date's clipped year range, including negative years. */
fn leap(year: i64) -> bool {
    year.rem_euclid(4) == 0 && (year.rem_euclid(100) != 0 || year.rem_euclid(400) == 0)
}
