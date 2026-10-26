/** Expansion is bounded by the array-length bound and the request's operation budget. */
use super::*;

/** A dense result may reach VM_ARRAY_LENGTH exactly; one more element is an uncatchable limit at the spread. */
#[test]
fn spread_results_admit_the_array_length_boundary_then_refuse_growth() {
    assert_eq!(
        result(
            &format!("const a=[];a.length={VM_ARRAY_LENGTH};const b=[...a];"),
            "b.length"
        ),
        VM_ARRAY_LENGTH.to_string()
    );
    let source =
        format!("const a=[];a.length={VM_ARRAY_LENGTH};try{{const b=[...a,1];}}catch(error){{}}");
    let error = execute(&source).unwrap_err();
    assert_eq!(error.code, "limit_exceeded");
    assert!(
        error.message.contains("array elements"),
        "{}",
        error.message
    );
    assert_eq!(error.phase.as_deref(), Some("execute"));
}

/** Prior work and later expansion share one decreasing allowance; no next operand gets to run. */
#[test]
fn expansion_uses_the_existing_request_operation_budget() {
    let source = "const a=[];a.length=1000;for(;;){const b=[...a];}";
    let error = execute(source).unwrap_err();
    assert_eq!(error.code, "limit_exceeded");
    assert_eq!(error.phase.as_deref(), Some("execute"));
}
