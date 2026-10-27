/** Date locale formatting coerces its options argument before formatting, as ToDateTimeOptions does: null throws, while an invalid date returns "Invalid Date" first. */
mod common;

/** The options coercion is observable through a thrown TypeError, and never runs for an invalid date. */
#[test]
fn null_locale_options_throw_before_formatting() {
    let r = common::eval(
        "let out=[];try{new Date(0).toLocaleDateString('de-DE',null);out.push(false)}catch(e){out.push(e instanceof TypeError)}\
        out.push(new Date(NaN).toLocaleDateString('en-US',null));out.push(new Date(0).toLocaleTimeString('en-US',undefined)!=='');\
        const r=out.join();",
        "r",
    );
    assert_eq!(r, "true,Invalid Date,true");
}
