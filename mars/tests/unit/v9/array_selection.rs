/** Predicate selection is observed through the same bounded public VM surfaces used by services. */
use mars::{
    constants::{VM_ARRAY_FIND_ID, VM_ARRAY_SOME_ID, VM_FUNCTION_INTRINSICS},
    v9::{VmState, event, execute, load},
};

/** Literal proposals expose results without depending on another JavaScript engine. */
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

/** Every retained state crosses a real private-wire round trip before the next event. */
fn next(source: &str, handler: &str, state: VmState) -> mars::v9::Execution {
    event(
        source,
        handler,
        serde_json::from_value(serde_json::to_value(state).unwrap()).unwrap(),
        &[],
    )
    .unwrap()
}

mod callbacks;
mod graph;
mod iteration;
