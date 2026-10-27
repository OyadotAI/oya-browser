/** Temporal.ZonedDateTime wall-clock getters, toString and toJSON over the shipped zone rules: one test per rule. */
mod common;
use common::eval;

/** The wall-clock year, month and day are read in the zone, not UTC: 02:00Z on July 1 is still June 30 in New York. */
#[test]
fn wall_clock_date_is_read_in_the_zone_not_utc() {
    let source = "const z = Temporal.Instant.from('2020-07-01T02:00:00Z').toZonedDateTimeISO('America/New_York');
        const out = [z.year, z.month, z.day].join();";
    assert_eq!(eval(source, "out"), "2020,6,30");
}

/** The wall-clock hour, minute and second follow the offset in force at that instant. */
#[test]
fn wall_clock_time_follows_the_offset_in_force() {
    let source = "const z = Temporal.Instant.from('2020-07-01T02:03:04Z').toZonedDateTimeISO('America/New_York');
        const out = [z.hour, z.minute, z.second].join();";
    assert_eq!(eval(source, "out"), "22,3,4");
}

/** The sub-second getters split the fraction into milliseconds, microseconds and nanoseconds. */
#[test]
fn sub_second_getters_split_the_fraction() {
    let source = "const z = Temporal.Instant.from('2020-07-01T02:00:00.123456789Z').toZonedDateTimeISO('UTC');
        const out = [z.millisecond, z.microsecond, z.nanosecond].join();";
    assert_eq!(eval(source, "out"), "123,456,789");
}

/** dayOfWeek is ISO numbering, Monday 1 to Sunday 7, taken from the wall date. */
#[test]
fn day_of_week_uses_the_wall_date() {
    let source = "const z = Temporal.Instant.from('2020-07-01T02:00:00Z').toZonedDateTimeISO('America/New_York');
        const out = String(z.dayOfWeek);";
    assert_eq!(eval(source, "out"), "2");
}

/** A wall-clock getter on a non-ZonedDateTime receiver is a TypeError, never a value read from another object. */
#[test]
fn wall_clock_getter_rejects_a_foreign_receiver() {
    let source = "let name = '';
        try { Object.getOwnPropertyDescriptor(Temporal.ZonedDateTime.prototype, 'year').get.call({}); } catch (e) { name = e.constructor.name; }
        const out = name;";
    assert_eq!(eval(source, "out"), "TypeError");
}

/** toString prints the wall time, the offset in force, and the zone in brackets. */
#[test]
fn zoned_to_string_prints_wall_time_offset_and_zone() {
    let source = "const out = Temporal.Instant.from('2020-07-01T02:00:00Z').toZonedDateTimeISO('America/New_York').toString();";
    assert_eq!(
        eval(source, "out"),
        "2020-06-30T22:00:00-04:00[America/New_York]"
    );
}

/** toString keeps a fraction of a second, with trailing zeros trimmed. */
#[test]
fn zoned_to_string_keeps_the_fraction() {
    let source = "const out = Temporal.Instant.from('2020-07-01T02:00:00.5Z').toZonedDateTimeISO('America/New_York').toString();";
    assert_eq!(
        eval(source, "out"),
        "2020-06-30T22:00:00.5-04:00[America/New_York]"
    );
}

/** A fixed-offset zone prints its offset and the offset again as the bracketed zone. */
#[test]
fn zoned_to_string_for_a_fixed_offset_zone() {
    let source = "const out = Temporal.Instant.from('2020-01-01T00:00Z').toZonedDateTimeISO('+05:30').toString();";
    assert_eq!(eval(source, "out"), "2020-01-01T05:30:00+05:30[+05:30]");
}

/** toJSON gives the same text as toString with default options. */
#[test]
fn zoned_to_json_matches_to_string() {
    let source = "const z = Temporal.Instant.from('2020-07-01T02:00:00Z').toZonedDateTimeISO('America/New_York');
        const out = z.toJSON() === z.toString() ? 'same' : 'differ';";
    assert_eq!(eval(source, "out"), "same");
}
