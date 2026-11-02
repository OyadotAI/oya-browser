/** Array-literal expansion preserves source effects, exact identities and independent resource caps. */
use mars::{
    constants::VM_ARRAY_LENGTH,
    v9::{VmState, event, execute, load},
};
use serde_json::json;

/** Native text proposals observe results without exposing interpreter internals. */
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

/** Private-wire round trips use the same snapshot codec as actual isolated service calls. */
fn round_trip(state: VmState) -> VmState {
    serde_json::from_value(serde_json::to_value(state).unwrap()).unwrap()
}

mod construction;
mod identity;
mod limits;
