/** Temporal.ZonedDateTime and Instant.prototype.toZonedDateTimeISO over the engine's shipped New York rules: one test per rule. */
use crate::common;
use common::eval;

/** The instant in the regression test reads as EDT, and the zone's canonical id is kept. */
#[test]
fn zoned_offset_is_daylight_time_in_summer_1969() {
    let source = "const z = Temporal.Instant.from('1969-07-20T20:17:00Z').toZonedDateTimeISO('America/New_York');
        const out = [z.offset, z.timeZoneId].join();";
    assert_eq!(eval(source, "out"), "-04:00,America/New_York");
}

/** Winter instants read as EST, before and after the table-to-footer handover. */
#[test]
fn zoned_offset_is_standard_time_in_winter() {
    let source = "const out = [
        Temporal.Instant.from('1969-01-15T12:00:00Z').toZonedDateTimeISO('America/New_York').offset,
        Temporal.Instant.from('2024-01-15T12:00:00Z').toZonedDateTimeISO('America/New_York').offset].join();";
    assert_eq!(eval(source, "out"), "-05:00,-05:00");
}

/** The offset changes exactly at the 2006 table's last autumn transition. */
#[test]
fn table_transition_is_exact_to_the_second() {
    let source =
        "const at = (s) => Temporal.Instant.from(s).toZonedDateTimeISO('America/New_York').offset;
        const out = [at('2006-10-29T05:59:59Z'), at('2006-10-29T06:00:00Z')].join();";
    assert_eq!(eval(source, "out"), "-04:00,-05:00");
}

/** The footer rule takes over in 2007 and moves DST to the second Sunday of March. */
#[test]
fn footer_rule_starts_dst_in_march_2007_at_the_exact_second() {
    let source =
        "const at = (s) => Temporal.Instant.from(s).toZonedDateTimeISO('America/New_York').offset;
        const out = [at('2007-03-11T06:59:59Z'), at('2007-03-11T07:00:00Z')].join();";
    assert_eq!(eval(source, "out"), "-05:00,-04:00");
}

/** The footer rule ends DST on the first Sunday of November at 2:00 local. */
#[test]
fn footer_rule_ends_dst_in_november() {
    let source =
        "const at = (s) => Temporal.Instant.from(s).toZonedDateTimeISO('America/New_York').offset;
        const out = [at('2007-11-04T05:59:59Z'), at('2007-11-04T06:00:00Z')].join();";
    assert_eq!(eval(source, "out"), "-04:00,-05:00");
}

/** Zone names match case-insensitively, but the value keeps the canonical identifier. */
#[test]
fn zone_name_is_matched_case_insensitively_and_canonicalized() {
    let source = "const z = Temporal.Instant.from('2020-07-01T00:00Z').toZonedDateTimeISO('america/new_york');
        const out = z.timeZoneId;";
    assert_eq!(eval(source, "out"), "America/New_York");
}

/** An unknown zone name is a RangeError, never a silent UTC fallback or a host lookup. */
#[test]
fn unknown_zone_name_is_a_range_error() {
    let source = "let name = '';
        try { Temporal.Instant.from('2020-07-01T00:00Z').toZonedDateTimeISO('Mars/Olympus'); } catch (e) { name = e.constructor.name; }
        const out = name;";
    assert_eq!(eval(source, "out"), "RangeError");
}

/** Instant.toString with a named zone prints the wall time and the offset in force at that instant. */
#[test]
fn instant_to_string_uses_the_named_zone_offset() {
    let source = "const out = Temporal.Instant.from('1969-07-20T20:17:00Z').toString({ timeZone: 'America/New_York' });";
    assert_eq!(eval(source, "out"), "1969-07-20T16:17:00-04:00");
}

/** A fixed offset still converts, with its own canonical identifier. */
#[test]
fn fixed_offset_zone_converts_without_rules() {
    let source =
        "const z = Temporal.Instant.from('2020-01-01T00:00Z').toZonedDateTimeISO('+05:30');
        const out = [z.offset, z.timeZoneId].join();";
    assert_eq!(eval(source, "out"), "+05:30,+05:30");
}

/** The exact instant survives the round trip through a ZonedDateTime. */
#[test]
fn zoned_keeps_the_exact_instant() {
    let source = "const i = Temporal.Instant.from('1969-07-20T20:17:00Z');
        const z = i.toZonedDateTimeISO('America/New_York');
        const out = [z.epochMilliseconds === i.epochMilliseconds, z.toInstant().equals(i),
            typeof z.epochNanoseconds].join();";
    assert_eq!(eval(source, "out"), "true,true,bigint");
}

/** The constructor reads the offset at the given instant and requires `new`. */
#[test]
fn zoned_constructor_reads_offset_at_its_instant() {
    let source = "const z = new Temporal.ZonedDateTime(0n, 'America/New_York');
        let threw = false;
        try { Temporal.ZonedDateTime(0n, 'America/New_York'); } catch (e) { threw = e instanceof TypeError; }
        const out = [z.offset, threw].join();";
    assert_eq!(eval(source, "out"), "-05:00,true");
}
