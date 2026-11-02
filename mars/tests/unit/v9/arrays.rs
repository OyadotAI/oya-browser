/** Source-guided array tests observe sparse ownership, callback ordering and bounded candidate publication. */
use mars::{
    constants::{JS_ENVIRONMENT_UNITS, JS_STRING_UNITS, VM_ARRAY_LENGTH},
    v9::{VmState, event, execute, load},
};
use serde_json::json;

/** Native text proposals make supported script values observable through the public interpreter facade. */
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

mod iteration;
mod join;
mod storage;
