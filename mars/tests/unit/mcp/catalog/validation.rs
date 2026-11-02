/** Semantic validation repeats required keys and byte bounds without rewriting native preconditions. */
use super::*;
use mars::constants::{ID_BYTES, TEXT_BYTES, URL_BYTES};
/** Fixed tool names map only to their reviewed native operation, preserving every provided value. */
#[test]
fn all_translations_copy_explicit_native_preconditions_unchanged() {
    let mut edit = target();
    edit["text"] = json!("literal\n<input>é");
    let mut checked = target();
    checked["checked"] = json!(false);
    let mut selected = target();
    selected["option_id"] = json!("native:option-11");
    let calls = [
        ("runtime_capabilities", "runtime.capabilities", json!({})),
        ("tab_list", "tab.list", json!({})),
        (
            "control_status",
            "control.status",
            json!({"tab_id":"native:tab-7"}),
        ),
        (
            "page_inspect",
            "page.inspect",
            json!({"tab_id":"native:tab-7"}),
        ),
        (
            "page_layout",
            "page.layout",
            json!({"tab_id":"native:tab-7"}),
        ),
        (
            "page_navigate",
            "page.navigate",
            json!({"tab_id":"native:tab-7","control_epoch":41,"url":"https://example.test/a?literal=é"}),
        ),
        ("page_type", "page.type", edit),
        ("page_click", "page.click", target()),
        ("page_set_checked", "page.set_checked", checked),
        ("page_select", "page.select", selected),
        ("page_submit", "page.submit", target()),
        (
            "page_back",
            "page.back",
            json!({"tab_id":"native:tab-7","control_epoch":41}),
        ),
        (
            "page_forward",
            "page.forward",
            json!({"tab_id":"native:tab-7","control_epoch":41}),
        ),
        (
            "page_reload",
            "page.reload",
            json!({"tab_id":"native:tab-7","control_epoch":41}),
        ),
        (
            "page_scroll",
            "page.scroll",
            json!({"tab_id":"native:tab-7","control_epoch":41,"page_id":"native:page-9","expected_revision":23,"y":-321.5}),
        ),
        (
            "control_release",
            "control.release",
            json!({"tab_id":"native:tab-7","control_epoch":41}),
        ),
    ];
    for (name, operation, arguments) in calls {
        let request = translate(name, &arguments, "mcp-73").unwrap();
        assert_eq!(request.id, "mcp-73");
        assert_eq!(request.op, operation);
        assert_eq!(request.params, arguments);
        request.validate().unwrap();
        for invalid_value in [Value::Null, json!([]), json!(true), json!("{}"), json!(7)] {
            invalid(name, &invalid_value);
        }
        let mut extra = arguments.clone();
        extra["unlisted_field"] = json!("private marker");
        invalid(name, &extra);
        for field in arguments.as_object().unwrap().keys() {
            let mut missing = arguments.clone();
            missing.as_object_mut().unwrap().remove(field);
            invalid(name, &missing);
            let mut wrong = arguments.clone();
            wrong[field] = Value::Null;
            invalid(name, &wrong);
        }
    }
}
/** No targeted discovery or mutation can silently choose a tab or omit an observed epoch. */
#[test]
fn targeted_tools_require_tab_and_native_mutation_preconditions() {
    for name in [
        "control_status",
        "page_inspect",
        "page_layout",
        "page_navigate",
        "page_type",
        "page_click",
        "page_set_checked",
        "page_select",
        "page_submit",
        "page_back",
        "page_forward",
        "page_reload",
        "page_scroll",
        "control_release",
    ] {
        invalid(name, &json!({}));
    }
    for field in ["tab_id", "page_id", "node_id"] {
        let mut args = target();
        args[field] = json!("");
        invalid("page_click", &args);
    }
    let mut selected = target();
    selected["option_id"] = json!("");
    invalid("page_select", &selected);
    let mut extra = target();
    extra["control_grant"] = json!(true);
    invalid("page_click", &extra);
}
/** Unsigned native counters retain their full range without string/float coercion; Boolean state stays explicit. */
#[test]
fn native_integer_boolean_and_scroll_types_are_not_coerced() {
    for field in ["control_epoch", "expected_revision"] {
        let mut arguments = target();
        arguments[field] = json!(u64::MAX);
        assert_eq!(
            translate("page_click", &arguments, "mcp-1").unwrap().params,
            arguments
        );
        for value in [json!(-1), json!(1.0), json!("1"), json!(true), Value::Null] {
            arguments[field] = value;
            invalid("page_click", &arguments);
        }
    }
    for value in [json!("true"), json!(1), Value::Null] {
        let mut arguments = target();
        arguments["checked"] = value;
        invalid("page_set_checked", &arguments);
    }
    for value in [json!("NaN"), json!(true), Value::Null] {
        invalid(
            "page_scroll",
            &json!({"tab_id":"t","control_epoch":1,"page_id":"p","expected_revision":0,"y":value}),
        );
    }
}
/** Multibyte strings cannot bypass native UTF-8 limits even when their character count fits JSON Schema. */
#[test]
fn semantic_byte_limits_reject_multibyte_overflow_without_truncation() {
    for field in ["tab_id", "page_id", "node_id"] {
        let mut arguments = target();
        arguments[field] = json!("é".repeat(ID_BYTES / 2));
        assert_eq!(
            translate("page_click", &arguments, "mcp-1").unwrap().params,
            arguments
        );
        arguments[field] = json!("é".repeat(ID_BYTES / 2 + 1));
        invalid("page_click", &arguments);
    }
    let mut arguments = target();
    arguments["option_id"] = json!("é".repeat(ID_BYTES / 2 + 1));
    invalid("page_select", &arguments);
    for (name, field, bound) in [
        ("page_type", "text", TEXT_BYTES),
        ("page_navigate", "url", URL_BYTES),
    ] {
        let mut arguments = if name == "page_type" {
            target()
        } else {
            json!({"tab_id":"t","control_epoch":0})
        };
        arguments[field] = json!("é".repeat(bound / 2));
        assert_eq!(
            translate(name, &arguments, "mcp-1").unwrap().params,
            arguments
        );
        arguments[field] = json!("é".repeat(bound / 2 + 1));
        invalid(name, &arguments);
    }
    let mut arguments = target();
    arguments["text"] = json!("");
    assert!(translate("page_type", &arguments, "mcp-1").is_ok());
}
/** Error copy is fixed and never includes raw target names, typed text or a rejected operation value. */
#[test]
fn invalid_argument_errors_do_not_echo_supplied_payloads() {
    let marker = "private-password-marker";
    let error = match translate(
        "page_type",
        &json!({"text":marker,"op":"runtime.shutdown"}),
        "mcp-1",
    ) {
        Err(TranslationError::InvalidArguments(error)) => error,
        _ => panic!("Expected argument refusal"),
    };
    let wire = serde_json::to_string(&error).unwrap();
    assert!(!wire.contains(marker));
    assert!(!wire.contains("runtime.shutdown"));
}
/** Agent tools accept their optional fields, keep values unchanged and refuse anything unlisted. */
#[test]
fn agent_tools_accept_optional_fields_and_refuse_out_of_bound_values() {
    let tab = json!({"tab_id":"native:tab-7"});
    let act = json!({"tab_id":"native:tab-7","control_epoch":41,"page_id":"native:page-9","expected_revision":23,"steps":[{"action":"click","node_id":"n"}]});
    for (name, operation, arguments) in [
        ("page_summary", "page.summary", tab.clone()),
        (
            "page_inspect",
            "page.inspect",
            json!({"tab_id":"t","fields":["role"],"visible_only":true,"compact":true}),
        ),
        (
            "page_find",
            "page.find",
            json!({"tab_id":"t","role":"button","name":"Go","label":"g","text":"o","limit":50,"visible_only":false}),
        ),
        (
            "page_diff",
            "page.diff",
            json!({"tab_id":"t","since_revision":3}),
        ),
        (
            "page_wait",
            "page.wait",
            json!({"tab_id":"t","text":"Done","timeout_ms":20000}),
        ),
        ("page_act", "page.act", act.clone()),
    ] {
        let request = translate(name, &arguments, "mcp-5").unwrap();
        assert_eq!(request.op, operation);
        assert_eq!(request.params, arguments);
        let mut extra = arguments.clone();
        extra["unlisted_field"] = json!(1);
        invalid(name, &extra);
    }
    let mut steps = act.clone();
    steps["steps"] = json!(vec![json!({"action":"scroll","y":0}); 17]);
    invalid("page_act", &steps);
    steps["steps"] = json!([]);
    invalid("page_act", &steps);
    steps["steps"] = json!([{"node_id":"n"}]);
    invalid("page_act", &steps);
    let mut mode = act.clone();
    mode["result"] = json!("everything");
    invalid("page_act", &mode);
    let mut epochless = act;
    epochless.as_object_mut().unwrap().remove("control_epoch");
    invalid("page_act", &epochless);
    invalid("page_wait", &json!({"tab_id":"t","timeout_ms":20001}));
    invalid("page_find", &json!({"tab_id":"t","limit":51}));
    invalid("page_find", &json!({"tab_id":"t","limit":0}));
    invalid("page_diff", &tab);
}
