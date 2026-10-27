/** The timeZone option must name a zone: values that are not zone names throw a RangeError, while UTC, offsets and Area/Location names are accepted. */
mod common;

/** A number is not a zone name, so the constructor throws a RangeError naming it. */
#[test]
fn numeric_time_zone_throws_range_error() {
    assert_eq!(
        common::eval(
            "let r;try{new Intl.DateTimeFormat('en-US',{timeZone:0});r='none'}catch(e){r=e.name+': '+e.message}",
            "r"
        ),
        "RangeError: Invalid time zone specified: 0"
    );
}

/** A boolean is not a zone name either. */
#[test]
fn boolean_time_zone_throws_range_error() {
    assert_eq!(
        common::eval(
            "let r;try{new Intl.DateTimeFormat('en-US',{timeZone:true});r='none'}catch(e){r=e.name}",
            "r"
        ),
        "RangeError"
    );
}

/** An object is converted with ToString first, so an object whose toString gives UTC is accepted. */
#[test]
fn object_time_zone_is_converted_before_validation() {
    assert_eq!(
        common::eval(
            "const zone={toString(){return 'UTC'}};const r=new Intl.DateTimeFormat('en-US',{timeZone:zone}).resolvedOptions().timeZone;",
            "r"
        ),
        "UTC"
    );
}

/** An Area/Location identifier is accepted and reported as written. */
#[test]
fn area_location_time_zone_is_accepted() {
    assert_eq!(
        common::eval(
            "const r=new Intl.DateTimeFormat('en-US',{timeZone:'America/New_York'}).resolvedOptions().timeZone;",
            "r"
        ),
        "America/New_York"
    );
}
