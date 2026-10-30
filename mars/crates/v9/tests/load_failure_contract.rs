/** Aggregate finalization failures travel independently of real indexed script failures. */
mod common;
use mars_v9::Execution;

#[test]
fn ordinary_script_failure_keeps_its_index_without_becoming_an_aggregate_failure() {
    let execution = common::load_with("throw new Error('stopped');", &common::environment());
    assert_eq!(execution.failed.len(), 1);
    assert_eq!(execution.failed[0].index, 0);
    assert!(execution.load_failure.is_none());
}

#[test]
fn aggregate_error_survives_resident_summary_serialization_without_inventing_script_failures() {
    let mut execution = common::load_with("var initialized = true;", &common::environment());
    execution.load_failure = Some(mars_core::protocol::EngineError::limit(
        "V9 event listeners",
        16384,
    ));
    execution.state = execution.state.summary();
    execution.state_token = Some(7);
    let encoded = serde_json::to_vec(&execution).unwrap();
    let restored: Execution = serde_json::from_slice(&encoded).unwrap();
    assert!(restored.failed.is_empty());
    let error = restored.load_failure.unwrap();
    assert_eq!(error.code, execution.load_failure.as_ref().unwrap().code);
    assert_eq!(
        error.message,
        execution.load_failure.as_ref().unwrap().message
    );
    assert!(error.location.is_none());
    assert_eq!(restored.state_token, Some(7));
}

#[test]
fn responses_without_the_optional_aggregate_field_remain_valid() {
    let execution = common::load_with("", &common::environment());
    let mut encoded = serde_json::to_value(&execution).unwrap();
    encoded.as_object_mut().unwrap().remove("load_failure");
    let restored: Execution = serde_json::from_value(encoded).unwrap();
    assert!(restored.load_failure.is_none());
}
