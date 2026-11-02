/** Public grammar and UTF-16 cases pin bounded array spread without exposing parser internals. */
mod grammar;
mod source_validation;
mod utf16;
use mars::v9::{JsString, JsValue, event, execute, load};

/** Existing DOM proposals expose results without introducing a test-only evaluator. */
fn result(source: &str, expression: &str) -> String {
    execute(&format!(
        "{source}document.getElementById('result').textContent={expression};"
    ))
    .unwrap()
    .remove(0)
    .text
}

/** Private-wire round trips prove raw string units survive ordinary state serialization. */
fn round_trip_units(source: &str, name: &str) -> Vec<u16> {
    let loaded = load(source, &[]).unwrap();
    let wire = serde_json::to_vec(&loaded.state).unwrap();
    let state: mars::v9::VmState = serde_json::from_slice(&wire).unwrap();
    let Some(JsValue::String(value)) = &state.globals()[name].value else {
        panic!("Expected a retained string value");
    };
    value.as_units().to_vec()
}
