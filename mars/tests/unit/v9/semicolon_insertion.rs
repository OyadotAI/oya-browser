/** Public V9 behavior pins grammar-level insertion without exposing scanner or parser internals. */
mod budgets;
mod continuation;
mod grammar;
mod restricted;
mod trivia;
mod validation;
use mars::v9::{event, execute, load, load_scripts};

/** A real DOM proposal observes expression values after semicolonless source statements. */
fn result(source: &str, expression: &str) -> String {
    execute(&format!(
        "{source}\ndocument.getElementById('result').textContent={expression}"
    ))
    .unwrap()
    .remove(0)
    .text
}

/** Failures retain their public classification and original parse location. */
fn syntax(source: &str) -> mars::protocol::EngineError {
    let error = load(source, &[]).unwrap_err();
    assert_eq!(error.code, "script_syntax_error", "{source}");
    assert_eq!(error.phase.as_deref(), Some("parse"), "{source}");
    assert!(error.location.is_some(), "{source}");
    error
}
