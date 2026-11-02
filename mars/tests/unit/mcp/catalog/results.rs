/** Result adaptation preserves full native JSON and keeps native failures distinct from protocol faults. */
use super::*;
use mars::{
    mcp::catalog::tool_result,
    protocol::{EngineError, SourceLocation},
};
/** A successful tool duplicates the complete native object without adding an assertion or filtering capabilities. */
#[test]
fn successful_native_objects_are_identical_in_text_and_structured_content() {
    let native = json!({"tab_id":"native:tab-1","nodes":[{"node_id":"native:node-4","label":"quote \" newline\n é"}],"operations":{"runtime.shutdown":{}},"revision":u64::MAX});
    let result = tool_result(Ok(native.clone())).unwrap();
    assert_eq!(result["isError"], false);
    assert_eq!(result["structuredContent"], native);
    assert_eq!(result["content"].as_array().unwrap().len(), 1);
    assert_eq!(result["content"][0]["type"], "text");
    assert_eq!(
        serde_json::from_str::<Value>(result["content"][0]["text"].as_str().unwrap()).unwrap(),
        native
    );
}
/** Every serialized native error field survives, including uncertainty/retention copy and source/field information. */
#[test]
fn native_errors_preserve_complete_machine_readable_details() {
    for code in [
        "control_denied",
        "revision_conflict",
        "validation_error",
        "submission_uncertain",
        "resubmission_required",
        "unsupported_feature",
        "service_unavailable",
    ] {
        let mut error = EngineError::new(
            code,
            "The native operation was refused.",
            "Inspect the result before deciding what to do next.",
        );
        error.retryable = true;
        error.previous_page_retained = true;
        error.service = Some("document".into());
        error.invalid_node = Some(Box::new(7));
        error.location = Some(Box::new(SourceLocation {
            byte_offset: 12,
            line: 2,
            column: 3,
            script_index: Some(1),
        }));
        error.phase = Some("execute".into());
        error.input_kind = Some("script".into());
        let expected = json!({"error":serde_json::to_value(&error).unwrap()});
        let result = tool_result(Err(error)).unwrap();
        assert_eq!(result["isError"], true);
        assert_eq!(result["structuredContent"], expected);
        assert_eq!(
            serde_json::from_str::<Value>(result["content"][0]["text"].as_str().unwrap()).unwrap(),
            expected
        );
    }
}
/** Malformed success payloads become internal protocol faults rather than partial or fabricated successful results. */
#[test]
fn nonobject_native_success_is_an_internal_failure() {
    for native in [
        Value::Null,
        json!(true),
        json!(0),
        json!("native private marker"),
        json!([]),
    ] {
        let error = tool_result(Ok(native)).unwrap_err();
        assert_eq!(error.code, "internal_error");
        assert!(
            !serde_json::to_string(&error)
                .unwrap()
                .contains("private marker")
        );
    }
}
