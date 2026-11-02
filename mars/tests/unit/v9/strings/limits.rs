/** Requested split limits and engine budgets remain independent and never cause silent truncation. */
use super::*;
use mars::{
    constants::VM_ARRAY_LENGTH,
    v9::{JsString, event},
};
/** Primitive limits truncate and reduce modulo uint32 rather than clamping to the engine's array cap. */
#[test]
fn split_limits_use_touint32() {
    for (limit, count) in [
        ("0", 0),
        ("-0", 0),
        ("NaN", 0),
        ("Infinity", 0),
        ("-Infinity", 0),
        ("null", 0),
        ("false", 0),
        ("true", 1),
        ("1.9", 1),
        ("'2'", 2),
        ("4294967297", 1),
        ("-4294967295", 1),
        ("-1", 3),
        ("undefined", 3),
    ] {
        assert_eq!(
            result("", &format!("'a,b,c'.split(',',{limit}).length")),
            count.to_string(),
            "{limit}"
        );
    }
    assert_eq!(result("", "'abc'.split(undefined,0).length"), "0");
}
/** The pure bounded algorithm accepts an exact capped request and rejects an actual extra requested field. */
#[test]
fn literal_split_checks_actual_output_before_allocating_an_excess_field() {
    let value = JsString::from_utf8(&",".repeat(VM_ARRAY_LENGTH)).unwrap();
    let separator = JsString::from_utf8(",").unwrap();
    let parts = value
        .split_literal(Some(&separator), VM_ARRAY_LENGTH as u32)
        .unwrap();
    assert_eq!(parts.len(), VM_ARRAY_LENGTH);
    assert!(parts.iter().all(JsString::is_empty));
    assert_eq!(
        value
            .split_literal(Some(&separator), u32::MAX)
            .unwrap_err()
            .code,
        "limit_exceeded"
    );
}
/** Repetitive near matches are linear and preserve literal matching outcomes. */
#[test]
fn literal_split_handles_repetitive_prefixes_without_losing_matches() {
    let value = JsString::from_utf8(&format!("{}b", "a".repeat(1024))).unwrap();
    let separator = JsString::from_utf8(&format!("{}b", "a".repeat(512))).unwrap();
    let parts = value.split_literal(Some(&separator), u32::MAX).unwrap();
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].len(), 512);
    assert!(parts[1].is_empty());
}
/** A failed split quota cannot publish earlier lexical or text mutations in the same event candidate. */
#[test]
fn split_failure_leaves_the_original_event_state_unchanged() {
    let source = format!(
        "let count=0;function run(){{count++;document.getElementById('out').textContent='changed';'{}'.split(',');}}",
        ",".repeat(VM_ARRAY_LENGTH)
    );
    let loaded = load(&source, &[]).unwrap();
    let before = serde_json::to_value(&loaded.state).unwrap();
    assert_eq!(
        event(&source, "run()", loaded.state.clone(), &[])
            .unwrap_err()
            .code,
        "limit_exceeded"
    );
    assert_eq!(serde_json::to_value(&loaded.state).unwrap(), before);
}
