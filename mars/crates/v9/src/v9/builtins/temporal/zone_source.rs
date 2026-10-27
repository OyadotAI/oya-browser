/** Parses the engine's embedded zone source: tzdata's zic rules, zones and links with offsets only. */
use std::collections::BTreeMap;

/** Month names; zic fields may abbreviate them to any unique prefix. */
const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
/** Weekday names, Sunday first, matched the same way as months. */
const WEEKDAYS: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];
/** Hours, minutes and seconds, the fields of a clock time in that order. */
const CLOCK_SCALES: [i64; 3] = [3600, 60, 1];
/** Stored as the rule end when a rule runs `max`; the index clamps it to its window. */
const RULE_END_MAX: i64 = i64::MAX;

/** A day in a month: a numbered day, or a weekday relative to a day, as zic writes it. */
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Day {
    /** The numbered day of the month. */
    Fixed(i64),
    /** The last given weekday of the month (0 is Sunday). */
    Last(i64),
    /** The first given weekday on or after a day. */
    OnOrAfter(i64, i64),
    /** The last given weekday on or before a day. */
    OnOrBefore(i64, i64),
}

/** Which offset converts a clock time to UTC: the wall clock in force, standard time, or UTC itself. */
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Basis {
    /** Wall-clock time: subtract the standard offset plus the save in force before the switch. */
    Wall,
    /** Standard time: subtract the standard offset only. */
    Standard,
    /** UTC: no conversion. */
    Utc,
}

/** A clock time within a day, in seconds, with the basis that converts it. */
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct At {
    /** Seconds from local midnight; zic allows hours past 24 and negative values. */
    pub seconds: i64,
    /** The offset basis of the time. */
    pub basis: Basis,
}

/** One rule line: a save change that recurs on a day of a month over a span of years. */
#[derive(Debug)]
pub(super) struct RuleLine {
    /** First year the rule applies. */
    pub from: i64,
    /** Last year the rule applies, or `RULE_END_MAX`. */
    pub to: i64,
    /** Month of the switch, 1 to 12. */
    pub month: i64,
    /** Day of the switch within the month. */
    pub on: Day,
    /** Clock time of the switch. */
    pub at: At,
    /** The save, in seconds, in force from the switch on. */
    pub save: i64,
}

/** Where an era's save comes from: a fixed amount, or the named rule set. */
#[derive(Debug)]
pub(super) enum Save {
    /** A constant save in seconds; zero for none. */
    Fixed(i64),
    /** The rule set with this name. */
    Named(String),
}

/** The local time at which an era gives way to the next. */
#[derive(Debug)]
pub(super) struct Until {
    /** The local year. */
    pub year: i64,
    /** The local month, 1 to 12. */
    pub month: i64,
    /** The local day. */
    pub day: Day,
    /** The local clock time and its basis. */
    pub at: At,
}

/** One era of a zone: a standard offset, a save source, and when it ends (absent for the last era). */
#[derive(Debug)]
pub(super) struct Era {
    /** The standard offset in seconds east of UTC. */
    pub stdoff: i64,
    /** The save source. */
    pub save: Save,
    /** When the era ends, if it is not the last. */
    pub until: Option<Until>,
}

/** A zone and its eras in order. */
#[derive(Debug)]
pub(super) struct ZoneLine {
    /** The canonical IANA name. */
    pub name: String,
    /** The eras, earliest first. */
    pub eras: Vec<Era>,
}

/** The whole parsed source: rule sets by name, zones, and (target, alias) links. */
#[derive(Default, Debug)]
pub(super) struct Source {
    /** Rule lines grouped by rule-set name, in file order. */
    pub rules: BTreeMap<String, Vec<RuleLine>>,
    /** Zones in file order. */
    pub zones: Vec<ZoneLine>,
    /** Links as (target zone, alias). */
    pub links: Vec<(String, String)>,
}

/** Parses the whole source; any malformed line fails the whole parse, so a bad table is never half used. */
pub(super) fn parse(text: &str) -> Option<Source> {
    let mut source = Source::default();
    for line in text.lines() {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        if !tokens.is_empty() {
            source.add(&tokens)?;
        }
    }
    Some(source)
}

impl Source {
    /** Adds one line: a rule, a link, a new zone, or an era continuing the last zone. */
    fn add(&mut self, tokens: &[&str]) -> Option<()> {
        match *tokens.first()? {
            "R" => {
                let rule = rule(tokens)?;
                self.rules.entry(tokens[1].into()).or_default().push(rule);
            }
            "L" => self
                .links
                .push((tokens.get(1)?.to_string(), tokens.get(2)?.to_string())),
            "Z" => self.zones.push(ZoneLine {
                name: tokens.get(1)?.to_string(),
                eras: vec![era(tokens.get(2..)?)?],
            }),
            _ => self.zones.last_mut()?.eras.push(era(tokens)?),
        }
        Some(())
    }
}

/** A rule line: `R name from to - in on at save`. */
fn rule(tokens: &[&str]) -> Option<RuleLine> {
    let [_, _, from, to, _, month, on, at, save] = tokens else {
        return None;
    };
    let from: i64 = from.parse().ok()?;
    Some(RuleLine {
        from,
        to: rule_end(to, from)?,
        month: month_number(month)?,
        on: day(on)?,
        at: clock(at)?,
        save: hms(save)?,
    })
}

/** The last year of a rule: `only` (or any prefix such as `o`) repeats the start year, `max` (or `ma`) runs open-ended. */
fn rule_end(text: &str, from: i64) -> Option<i64> {
    if !text.is_empty() && "only".starts_with(text) {
        return Some(from);
    }
    if !text.is_empty() && "max".starts_with(text) {
        return Some(RULE_END_MAX);
    }
    text.parse().ok()
}

/** An era: `stdoff rules [until...]`, where rules is `-`, a fixed save, or a rule-set name. */
fn era(tokens: &[&str]) -> Option<Era> {
    let stdoff = hms(tokens.first()?)?;
    let save = match *tokens.get(1)? {
        "-" => Save::Fixed(0),
        text => match hms(text) {
            Some(fixed) => Save::Fixed(fixed),
            None => Save::Named(text.into()),
        },
    };
    let until = match tokens.get(2..) {
        Some(rest) if !rest.is_empty() => Some(until(rest)?),
        _ => None,
    };
    Some(Era {
        stdoff,
        save,
        until,
    })
}

/** An until: `year [month [day [time]]]`, with month, day and time defaulting to January 1st at midnight. */
fn until(tokens: &[&str]) -> Option<Until> {
    let year: i64 = tokens.first()?.parse().ok()?;
    let mut rest = &tokens[1..];
    let month = take(&mut rest, month_number).unwrap_or(1);
    let day = take(&mut rest, day).unwrap_or(Day::Fixed(1));
    let at = take(&mut rest, clock).unwrap_or(At {
        seconds: 0,
        basis: Basis::Wall,
    });
    Some(Until {
        year,
        month,
        day,
        at,
    })
}

/** Consumes the next token when `parse` accepts it, leaving it in place otherwise. */
fn take<T>(rest: &mut &[&str], parse: impl Fn(&str) -> Option<T>) -> Option<T> {
    let value = parse(rest.first()?)?;
    *rest = &rest[1..];
    Some(value)
}

/** A month name or unique prefix, as a number from 1 to 12. */
fn month_number(text: &str) -> Option<i64> {
    prefixed(text, &MONTHS).map(|index| index as i64 + 1)
}

/** A day field: a number, `lastSu`, `Sun>=8`, or `Sun<=25`. */
fn day(text: &str) -> Option<Day> {
    if let Some(name) = text.strip_prefix("last") {
        return Some(Day::Last(weekday(name)?));
    }
    if let Some((name, number)) = text.split_once(">=") {
        return Some(Day::OnOrAfter(weekday(name)?, number.parse().ok()?));
    }
    if let Some((name, number)) = text.split_once("<=") {
        return Some(Day::OnOrBefore(weekday(name)?, number.parse().ok()?));
    }
    text.parse().ok().map(Day::Fixed)
}

/** A weekday name or unique prefix, as an index with Sunday at 0. */
fn weekday(text: &str) -> Option<i64> {
    prefixed(text, &WEEKDAYS).map(|index| index as i64)
}

/** The one name that starts with `text`, ignoring case; an empty or ambiguous prefix matches nothing. */
fn prefixed(text: &str, names: &[&str]) -> Option<usize> {
    let mut hits = names
        .iter()
        .enumerate()
        .filter(|(_, name)| !text.is_empty() && starts_with_ignore_case(name, text));
    let (index, _) = hits.next()?;
    hits.next().is_none().then_some(index)
}

/** Case-insensitive prefix test for ASCII names. */
fn starts_with_ignore_case(name: &str, prefix: &str) -> bool {
    name.as_bytes()
        .get(..prefix.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(prefix.as_bytes()))
}

/** A clock time with an optional `u`, `s` or `w` suffix naming its basis; no suffix means wall time. */
fn clock(text: &str) -> Option<At> {
    let (body, basis) = match text.as_bytes().last()? {
        b'u' | b'g' | b'z' => (&text[..text.len() - 1], Basis::Utc),
        b's' => (&text[..text.len() - 1], Basis::Standard),
        b'w' => (&text[..text.len() - 1], Basis::Wall),
        _ => (text, Basis::Wall),
    };
    Some(At {
        seconds: hms(body)?,
        basis,
    })
}

/** An optionally signed `h[:m[:s]]` value in seconds; a leading minus negates all of it. */
fn hms(text: &str) -> Option<i64> {
    let (sign, body) = match text.strip_prefix('-') {
        Some(rest) => (-1, rest),
        None => (1, text),
    };
    let fields: Vec<i64> = body
        .split(':')
        .map(|part| part.parse().ok())
        .collect::<Option<_>>()?;
    if fields.len() > CLOCK_SCALES.len() {
        return None;
    }
    let total: i64 = fields
        .iter()
        .zip(CLOCK_SCALES)
        .map(|(field, scale)| field * scale)
        .sum();
    Some(sign * total)
}
