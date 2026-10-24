/** Public consumers execute V9 without importing Eye, Relay or the Mars application. */
use mars_v9::{JsString, JsValue, load};

/** Automatic separators and code-point spread remain available through the standalone facade. */
#[test]
fn evaluates_asi_and_array_string_spread_without_services() {
    let execution = load(
        "const prefix = ['Mars']\nconst label = [...prefix, ...'A😀B'].join('|')\nconst count = [...'A😀B'].length",
        &[],
    )
    .unwrap();
    assert_eq!(
        execution.state.globals()["label"].value,
        Some(JsValue::string("Mars|A|😀|B").unwrap())
    );
    assert_eq!(
        execution.state.globals()["count"].value,
        Some(JsValue::Number(3.0))
    );
    assert!(execution.changes.is_empty());
}

/** A library caller receives the same located syntax failure as the composed browser. */
#[test]
fn rejects_invalid_source_through_the_public_facade() {
    let error = load("const missing =", &[]).unwrap_err();
    assert_eq!(error.code, "script_syntax_error");
    assert_eq!(error.phase.as_deref(), Some("parse"));
    assert!(error.location.is_some());
}

/** Spreading a non-iterable object is a TypeError, as in browsers, reported with its location. */
#[test]
fn spreading_a_plain_object_throws_a_located_type_error() {
    let error = load("const values = [...{}]", &[]).unwrap_err();
    assert_eq!(error.code, "script_execution_error");
    assert!(error.message.contains("TypeError"));
    assert!(error.message.contains("not iterable"));
    assert_eq!(error.phase.as_deref(), Some("execute"));
    assert!(error.location.is_some());
}

/** The public borrowed cursor preserves paired and unpaired UTF-16 without DOM conversion. */
#[test]
fn exposes_lossless_bounded_code_point_iteration() {
    let text = JsString::from_units(vec![0xd83d, 0xde00, 0xd800]).unwrap();
    let points: Vec<&[u16]> = text.code_points().collect();
    assert_eq!(points, vec![&[0xd83d, 0xde00][..], &[0xd800][..]]);
    assert!(text.to_dom().is_err());
}
