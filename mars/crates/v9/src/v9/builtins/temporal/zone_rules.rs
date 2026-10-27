/** Evaluates a zone's eras and rule sets into UTC transitions over a fixed window, folding later instants by whole 400-year cycles. */
use super::iso;
use super::zone_source::{Basis, Day, Era, RuleLine, Save, Until};
use std::collections::BTreeMap;

/** Rule sets by name, as the source parser produces them. */
pub(super) type RuleTable = BTreeMap<String, Vec<RuleLine>>;

/** Seconds in a day, to turn epoch days into epoch seconds. */
const SECONDS_PER_DAY: i64 = 86_400;
/** Days in a week, for weekday arithmetic. */
const DAYS_PER_WEEK: i64 = 7;
/** Weekday of epoch day 0, 1970-01-01, a Thursday, with Sunday at 0. */
const EPOCH_WEEKDAY: i64 = 4;
/** First year whose switches are generated; earlier instants take the first era's offset. */
pub(super) const FIRST_YEAR: i64 = 1800;
/** Switches are generated for years before this one; the window is [2100, 2500). Every rule and era boundary in the source falls before 2100, so the window repeats exactly. */
pub(super) const WINDOW_END_YEAR: i64 = 2500;
/** The Gregorian calendar repeats every 400 years, so instants past the window fold back by this span. */
pub(super) const CYCLE_YEARS: i64 = 400;
/** Years of switches before an era starts, so the DST state the era begins in is known. */
const LEAD_IN_YEARS: i64 = 2;

/** A switch as a rule produces it: nominal local time, the basis that converts it, and the save it sets. */
struct Event {
    /** Seconds from the epoch in local time, before any save is subtracted. */
    local: i64,
    /** Which offset converts `local` to UTC. */
    basis: Basis,
    /** The save in force from this switch on. */
    save: i64,
    /** The switch's UTC instant, filled in once the events are ordered. */
    utc: i64,
}

/** Folds an instant past the window into [2100, 2500) by whole 400-year cycles, which repeat exactly. */
pub(super) fn fold(epoch_seconds: i64) -> i64 {
    let end = seconds_of_year(WINDOW_END_YEAR);
    if epoch_seconds < end {
        return epoch_seconds;
    }
    let start = seconds_of_year(WINDOW_END_YEAR - CYCLE_YEARS);
    let cycle = end - start;
    epoch_seconds - (epoch_seconds - start).div_euclid(cycle) * cycle
}

/** The epoch second of January 1st of a year. */
fn seconds_of_year(year: i64) -> i64 {
    iso::epoch_days(year, 1, 1) * SECONDS_PER_DAY
}

/** A zone's transitions as (epoch seconds, offset seconds from then on), sorted, led by a sentinel at the earliest instant. */
pub(super) fn transitions(eras: &[Era], rules: &RuleTable) -> Vec<(i64, i32)> {
    let mut out = Vec::new();
    let mut start = i64::MIN;
    for era in eras {
        let Some(end) = add_era(&mut out, start, era, rules) else {
            break;
        };
        start = end;
    }
    out
}

/** Appends one era's transitions; returns the instant it ends, or None when it runs open-ended. */
fn add_era(out: &mut Vec<(i64, i32)>, start: i64, era: &Era, rules: &RuleTable) -> Option<i64> {
    let fixed = match era.save {
        Save::Fixed(save) => save,
        Save::Named(_) => 0,
    };
    let events = match &era.save {
        Save::Fixed(_) => Vec::new(),
        Save::Named(name) => {
            let set = rules.get(name).map_or(&[][..], Vec::as_slice);
            era_events(set, start, era.stdoff, era.until.as_ref())
        }
    };
    let end = era
        .until
        .as_ref()
        .map(|until| until_utc(until, era.stdoff, fixed, &events));
    let save_at_start = events
        .iter()
        .take_while(|event| event.utc < start)
        .last()
        .map_or(fixed, |event| event.save);
    push(out, start, era.stdoff + save_at_start);
    for event in events.iter().filter(|event| event.utc >= start) {
        if end.is_some_and(|stop| event.utc >= stop) {
            break;
        }
        push(out, event.utc, era.stdoff + event.save);
    }
    end
}

/** The events of one rule set that can affect an era, ordered by UTC instant. */
fn era_events(set: &[RuleLine], start: i64, stdoff: i64, until: Option<&Until>) -> Vec<Event> {
    let first = if start == i64::MIN {
        FIRST_YEAR
    } else {
        (year_of(start) - LEAD_IN_YEARS).max(FIRST_YEAR)
    };
    let last = until.map_or(WINDOW_END_YEAR - 1, |until| {
        (until.year + 1).min(WINDOW_END_YEAR - 1)
    });
    let mut events: Vec<Event> = (first..=last)
        .flat_map(|year| {
            set.iter()
                .filter(move |rule| rule.from <= year && year <= rule.to)
                .map(move |rule| event(year, rule))
        })
        .collect();
    events.sort_by_key(|event| event.local);
    let mut prev_save = 0;
    for event in &mut events {
        event.utc = event.local - convert(event.basis, stdoff, prev_save);
        prev_save = event.save;
    }
    events.sort_by_key(|event| event.utc);
    events
}

/** The event a rule produces in a given year. */
fn event(year: i64, rule: &RuleLine) -> Event {
    let day = resolve(year, rule.month, rule.on);
    Event {
        local: day * SECONDS_PER_DAY + rule.at.seconds,
        basis: rule.at.basis,
        save: rule.save,
        utc: 0,
    }
}

/** The seconds subtracted from local time to reach UTC, for a basis, standard offset and save in force. */
fn convert(basis: Basis, stdoff: i64, save: i64) -> i64 {
    match basis {
        Basis::Wall => stdoff + save,
        Basis::Standard => stdoff,
        Basis::Utc => 0,
    }
}

/** The UTC instant at which an era ends, from its local until time and the save in force there. */
fn until_utc(until: &Until, stdoff: i64, fixed: i64, events: &[Event]) -> i64 {
    let local = resolve(until.year, until.month, until.day) * SECONDS_PER_DAY + until.at.seconds;
    let save = events
        .iter()
        .rfind(|event| event.local < local)
        .map_or(fixed, |event| event.save);
    local - convert(until.at.basis, stdoff, save)
}

/** Appends a transition unless the offset is unchanged; a second transition at the same instant replaces the first. */
fn push(out: &mut Vec<(i64, i32)>, at: i64, offset: i64) {
    let offset = offset as i32;
    match out.last_mut() {
        Some(last) if last.0 == at => last.1 = offset,
        Some(last) if last.1 == offset => {}
        _ => out.push((at, offset)),
    }
}

/** The epoch day a day specification names in a month of a year. */
fn resolve(year: i64, month: i64, day: Day) -> i64 {
    match day {
        Day::Fixed(number) => iso::epoch_days(year, month, number),
        Day::Last(weekday) => {
            let last = iso::epoch_days(year, month, iso::days_in_month(year, month));
            last - (weekday_of(last) - weekday).rem_euclid(DAYS_PER_WEEK)
        }
        Day::OnOrAfter(weekday, number) => {
            let from = iso::epoch_days(year, month, number);
            from + (weekday - weekday_of(from)).rem_euclid(DAYS_PER_WEEK)
        }
        Day::OnOrBefore(weekday, number) => {
            let from = iso::epoch_days(year, month, number);
            from - (weekday_of(from) - weekday).rem_euclid(DAYS_PER_WEEK)
        }
    }
}

/** The weekday of an epoch day, with Sunday at 0. */
fn weekday_of(days: i64) -> i64 {
    (days + EPOCH_WEEKDAY).rem_euclid(DAYS_PER_WEEK)
}

/** The civil year containing a UTC instant. */
fn year_of(epoch_seconds: i64) -> i64 {
    iso::from_epoch_days(epoch_seconds.div_euclid(SECONDS_PER_DAY)).0
}
