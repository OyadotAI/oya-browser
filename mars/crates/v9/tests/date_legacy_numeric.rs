/** Legacy numeric date strings support explicit GMT and validated numeric calendar fields. */
use crate::common;

/** A UTC designator on an extended signed date must preserve the sign and full year. */
#[test]
fn signed_extended_dates_with_utc_designator_keep_the_year() {
    assert_eq!(
        common::eval(
            "const r=[Date.parse('-000040-01-01Z'),Date.parse('+001972Z'),Date.parse('0040-01-01Z')].join();",
            "r"
        ),
        "-63429523200000,63072000000,-60904915200000"
    );
}

/** GMT follows year-first numeric dates without changing the date into local time. */
#[test]
fn year_first_gmt_numeric_dates_parse() {
    assert_eq!(
        common::eval(
            "const r=new Date('1979-01-11 08:00 GMT').toISOString();",
            "r"
        ),
        "1979-01-11T08:00:00.000Z"
    );
}

/** Slash-separated legacy forms support year-first and month-first ordering. */
#[test]
fn slash_dates_support_both_numeric_orders() {
    assert_eq!(
        common::eval(
            "const r=[new Date('2000/01/02 08:00 UT').toISOString(),new Date('01/02/2000 08:00 GMT').toISOString()].join();",
            "r"
        ),
        "2000-01-02T08:00:00.000Z,2000-01-02T08:00:00.000Z"
    );
}

/** Out-of-range numeric fields fail rather than silently dropping nonnumeric components. */
#[test]
fn malformed_numeric_dates_are_invalid() {
    assert_eq!(
        common::eval(
            "const r=['13/01/2000 GMT','01/32/2000 GMT','01/no/2000 GMT'].every(s=>Number.isNaN(Date.parse(s)));",
            "r"
        ),
        "true"
    );
}

/** Legacy month words tolerate prefixes and nested comments, while timezone keywords adjust real UTC offsets. */
#[test]
fn english_tokens_preserve_comments_month_prefixes_and_zones() {
    assert_eq!(
        common::eval(
            "const r=[Date.parse('Ignore this stuff 01-Janisamonth-2000 08:00:00 UT'),Date.parse('Sat()(2001)01 Jan 2000 03:00:00 EST'),Date.parse('Jan01 2000 08:00 GMT+0')].join();",
            "r"
        ),
        "946713600000,946713600000,946713600000"
    );
}

/** Unknown words after numeric date input and unmatched closing comments are rejected. */
#[test]
fn malformed_legacy_tokens_do_not_silently_disappear() {
    assert_eq!(
        common::eval(
            "const r=['a1','x_2','May 25 2008 AAA (GMT)','May 25 2008 1:30 (PM)) UTC','May 25 2008 1:30( )AM (PM)'].every(s=>Number.isNaN(Date.parse(s)));",
            "r"
        ),
        "true"
    );
}

/** ISO fields must be valid before normalization; only an exactly empty 24th hour is accepted. */
#[test]
fn iso_calendar_and_clock_fields_are_validated() {
    assert_eq!(
        common::eval(
            "const bad=['2000-13-01','2000-01-32','2000-01-01T24:00:01Z','2000-01-01T23:60:00Z','2000-01-01T00:00+24:00','2000-01-01T00:00+01:000','-000000-01-01'];const r=[bad.every(s=>Number.isNaN(Date.parse(s))),new Date('2000-01-01T24:00Z').toISOString()].join('|');",
            "r"
        ),
        "true|2000-01-02T00:00:00.000Z"
    );
}
