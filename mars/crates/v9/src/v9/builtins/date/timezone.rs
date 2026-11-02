/** Date timezone conversion consumes validated host transitions and POSIX recurrence without filesystem or process-global state. */
use super::*;
use crate::v9::platform::PageEnvironment;
mod rule;

/** Host transition data is small, inert metadata rather than a replacement for runtime heap bounds. */
const MAX_TRANSITIONS: usize = 4096;
/** POSIX timezone footers have a bounded ASCII grammar. */
const MAX_RULE_BYTES: usize = 256;
/** Timezone offsets never exceed a day in either direction. */
const MAX_OFFSET_SECONDS: i32 = 86400;
/** Display names are bounded metadata and never authorize a native timezone provider. */
const MAX_ZONE_NAME_UNITS: usize = 1024;

/** Reject malformed transition authority before opening a VM or running any script. */
pub(in crate::v9) fn validate(environment: &PageEnvironment) -> crate::protocol::EngineResult<()> {
    let transitions = &environment.timezone_transitions;
    let valid = (environment.timezone_daylight.is_empty()
        || environment.timezone_daylight.len() == transitions.len())
        && environment
            .timezone_display_names
            .as_ref()
            .is_none_or(|(standard, daylight)| {
                standard.encode_utf16().count() <= MAX_ZONE_NAME_UNITS
                    && daylight.encode_utf16().count() <= MAX_ZONE_NAME_UNITS
            })
        && environment.timezone_offset_minutes.is_finite()
        && environment.timezone_offset_minutes.abs() <= f64::from(MAX_OFFSET_SECONDS) / 60.0
        && transitions.len() <= MAX_TRANSITIONS
        && transitions.iter().all(|(time, offset)| {
            time.is_finite()
                && time.abs() <= MAX_TIME
                && time.fract() == 0.0
                && offset.unsigned_abs() <= MAX_OFFSET_SECONDS as u32
        })
        && transitions.windows(2).all(|pair| pair[0].0 < pair[1].0)
        && environment
            .timezone_initial_offset_seconds
            .is_none_or(|offset| offset.unsigned_abs() <= MAX_OFFSET_SECONDS as u32)
        && environment
            .timezone_posix_rule
            .as_ref()
            .is_none_or(|text| text.len() <= MAX_RULE_BYTES && rule::Rule::parse(text).is_some());
    if valid {
        Ok(())
    } else {
        Err(crate::protocol::EngineError::invalid(
            "Invalid Date timezone transition data.",
        ))
    }
}

/** The offset effective at a UTC instant uses binary search and then the recurring future rule. */
pub(super) fn offset(environment: &PageEnvironment, time: f64) -> f64 {
    if !time.is_finite() {
        return f64::NAN;
    }
    let transitions = &environment.timezone_transitions;
    if transitions.last().is_none_or(|last| time > last.0)
        && let Some(rule) = environment
            .timezone_posix_rule
            .as_deref()
            .and_then(rule::Rule::parse)
    {
        return f64::from(rule.offset(time)) * SECOND;
    }
    let index = transitions.partition_point(|(at, _)| *at <= time);
    if index > 0 {
        f64::from(transitions[index - 1].1) * SECOND
    } else {
        initial(environment)
    }
}

/** Local conversion selects the earlier occurrence of a fold and advances a gap by its actual offset change. */
pub(super) fn utc(environment: &PageEnvironment, local: f64) -> f64 {
    if !local.is_finite() {
        return f64::NAN;
    }
    let mut offsets = vec![initial(environment)];
    offsets.extend(
        environment
            .timezone_transitions
            .iter()
            .map(|(_, offset)| f64::from(*offset) * SECOND),
    );
    let recurring = environment
        .timezone_posix_rule
        .as_deref()
        .and_then(rule::Rule::parse);
    if let Some(rule) = &recurring {
        offsets.push(f64::from(rule.standard) * SECOND);
        offsets.push(f64::from(rule.daylight) * SECOND);
    }
    offsets.sort_by(f64::total_cmp);
    offsets.dedup();
    if let Some(utc) = offsets
        .iter()
        .map(|off| local - off)
        .filter(|utc| offset(environment, *utc) == local - *utc)
        .min_by(f64::total_cmp)
    {
        return utc;
    }
    gap(environment, local, recurring.as_ref()).unwrap_or(local - initial(environment))
}

/** A nonexistent wall time is interpreted with the offset immediately before its forward transition. */
fn gap(environment: &PageEnvironment, local: f64, recurring: Option<&rule::Rule>) -> Option<f64> {
    let mut prior = initial(environment);
    for (at, seconds) in &environment.timezone_transitions {
        let after = f64::from(*seconds) * SECOND;
        if after > prior && local >= at + prior && local < at + after {
            return Some(local - prior);
        }
        prior = after;
    }
    let rule = recurring?;
    let year = parts(local).year as i64;
    for year in year - 1..=year + 1 {
        for (at, before, after) in rule.events(year) {
            if after > before
                && local >= at + f64::from(before) * SECOND
                && local < at + f64::from(after) * SECOND
            {
                return Some(local - f64::from(before) * SECOND);
            }
        }
    }
    None
}

/** Fixed-offset hosts retain their existing minute precision when no initial TZif offset is supplied. */
fn initial(environment: &PageEnvironment) -> f64 {
    environment
        .timezone_initial_offset_seconds
        .map_or(environment.timezone_offset_minutes * MINUTE, |offset| {
            f64::from(offset) * SECOND
        })
}

/** Localized names follow actual daylight flags rather than guessing from an offset or city name. */
pub(super) fn display_name(environment: &PageEnvironment, time: f64) -> Option<&str> {
    let (standard, daylight) = environment.timezone_display_names.as_ref()?;
    let future = environment
        .timezone_transitions
        .last()
        .is_none_or(|last| time > last.0);
    let is_daylight = if future
        && let Some(rule) = environment
            .timezone_posix_rule
            .as_deref()
            .and_then(rule::Rule::parse)
    {
        rule.is_daylight(time)
    } else {
        let index = environment
            .timezone_transitions
            .partition_point(|(at, _)| *at <= time);
        if index == 0 {
            environment.timezone_initial_daylight
        } else {
            environment
                .timezone_daylight
                .get(index - 1)
                .copied()
                .unwrap_or(false)
        }
    };
    Some(if is_daylight { daylight } else { standard })
}
