/** String methods are tested through public V9 execution and UTF-16 value inspection. */
mod conversion;
mod limits;
mod literal;
use mars::v9::{JsValue, VmState, execute, load};
/** Native text proposals expose method results without leaking private heap records. */
fn result(source: &str, expression: &str) -> String {
    execute(&format!(
        "{source}document.getElementById('out').textContent={expression};"
    ))
    .unwrap()
    .last()
    .unwrap()
    .text
    .clone()
}
/** Retained lexical strings preserve otherwise unrepresentable surrogate units. */
fn units(state: &VmState, name: &str) -> Vec<u16> {
    let Some(JsValue::String(value)) = state
        .globals()
        .get(name)
        .and_then(|cell| cell.value.as_ref())
    else {
        panic!("expected retained string");
    };
    value.as_units().to_vec()
}
