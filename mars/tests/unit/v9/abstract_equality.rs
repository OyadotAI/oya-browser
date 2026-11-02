/** Public script boundaries pin loose equality without granting an arbitrary evaluator. */
use mars::v9::{execute, load};

/** Literal document proposals expose ordinary expression results through the supported facade. */
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

mod failures;
mod identities;
mod primitives;
