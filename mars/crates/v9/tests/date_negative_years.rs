/** DateString pads a negative year's magnitude to four digits before adding its sign. */
use crate::common;

/** toUTCString and toDateString print years before 1 BCE as -0001 through -123456. */
#[test]
fn negative_years_pad_the_magnitude_to_four_digits() {
    let source = "const years=['-000001','-000012','-000123','-001234','-012345','-123456'];\
        const r=years.map(y=>{const d=new Date(y+'-07-01T00:00Z');\
        return d.toUTCString().split(' ')[3]+'/'+d.toDateString().split(' ')[3]}).join();";
    assert_eq!(
        common::eval(source, "r"),
        "-0001/-0001,-0012/-0012,-0123/-0123,-1234/-1234,-12345/-12345,-123456/-123456"
    );
}

/** Positive years keep their four-digit padding. */
#[test]
fn positive_years_keep_four_digits() {
    let source = "const r=new Date('0012-07-01T00:00Z').toUTCString().split(' ')[3];";
    assert_eq!(common::eval(source, "r"), "0012");
}
