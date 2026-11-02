/** Tool discovery preserves the reviewed names, exact descriptions and complete strict input surface. */
use super::*;
use mars::{
    constants::{ID_BYTES, TEXT_BYTES, URL_BYTES},
    mcp::catalog::tools,
};
/** The public catalog exposes exactly the native subset and designer wording. */
#[test]
fn discovery_contains_exact_fixed_tools_and_descriptions() {
    let expected = [
        (
            "runtime_capabilities",
            "List this native runtime's supported features and limits.",
        ),
        ("tab_list", "List native tabs and their identities."),
        (
            "control_status",
            "Read ownership and the current control epoch for one native tab.",
        ),
        (
            "page_inspect",
            "Inspect one native tab's committed page, controls and current identities.",
        ),
        (
            "page_summary",
            "Read a short outline of one tab's page: title, headings, landmarks and numbered controls with their current values.",
        ),
        (
            "page_find",
            "Find nodes by role, accessible name, label or text and return their node IDs and bounds.",
        ),
        (
            "page_diff",
            "List the nodes added, removed or changed on the current page since an earlier revision.",
        ),
        (
            "page_wait",
            "Wait until one condition holds on the tab's page, or until the timeout passes.",
        ),
        (
            "page_act",
            "Run several steps in order on one tab, each against the revision the last step produced. Stops at the first failure.",
        ),
        (
            "page_layout",
            "Read the native display geometry for one tab in logical points.",
        ),
        (
            "page_navigate",
            "Load a supported URL in the specified native tab.",
        ),
        (
            "page_type",
            "Replace an enabled editable field's value with literal text.",
        ),
        (
            "page_click",
            "Activate a supported native link or control by its discovered node identity.",
        ),
        (
            "page_set_checked",
            "Set a supported checkbox or radio control's checked state explicitly.",
        ),
        (
            "page_select",
            "Select an enabled option using its discovered native option identity.",
        ),
        (
            "page_submit",
            "Submit the targeted form using the native validation and submitter rules.",
        ),
        (
            "page_back",
            "Go back in the specified tab, respecting native form-resubmission safeguards.",
        ),
        (
            "page_forward",
            "Go forward in the specified tab, respecting native form-resubmission safeguards.",
        ),
        (
            "page_reload",
            "Reload the specified tab when its current page can be replayed safely.",
        ),
        (
            "page_scroll",
            "Set the document's vertical scroll offset in logical points.",
        ),
        (
            "tab_open",
            "Open a background tab of your own, under your control, optionally on a URL. Returns its tab_id and control_epoch. The person's tabs are untouched.",
        ),
        (
            "tab_close",
            "Close a tab you opened with tab_open. Tabs you did not open are refused.",
        ),
        (
            "page_screenshot",
            "Capture the tab's visible viewport, or one element's box, as a PNG image.",
        ),
        (
            "page_click_at",
            "Click whatever a person would click at a viewport point. Reports the node that was hit.",
        ),
        (
            "page_hover",
            "Read the node under a viewport point and what a click there would reach. Changes nothing.",
        ),
        (
            "page_scroll_at",
            "Turn the wheel at a viewport point, scrolling the page by delta_y points.",
        ),
        (
            "page_press",
            "Press a key on a field or control (Enter submits, Space toggles, Backspace erases), scroll with PageDown/PageUp/Home/End, or append text. Give exactly one of key or text.",
        ),
        (
            "page_console",
            "Read the engine's recent messages about this page: skipped scripts and refused resources.",
        ),
        (
            "page_network",
            "List the loads behind the current page: the document with method and status, and images with type and size. No headers or cookies.",
        ),
        (
            "control_release",
            "Return agent control of the specified native tab to the person.",
        ),
    ];
    let catalog = tools();
    assert_eq!(catalog.len(), expected.len());
    for (tool, (name, description)) in catalog.iter().zip(expected) {
        assert_eq!(tool["name"], name);
        assert_eq!(tool["description"], description);
        assert_eq!(tool["inputSchema"]["type"], "object");
        assert_eq!(tool["inputSchema"]["additionalProperties"], false);
        assert!(tool.get("outputSchema").is_none());
        let required = tool["inputSchema"]["required"].as_array().unwrap();
        let properties = tool["inputSchema"]["properties"].as_object().unwrap();
        assert!(required.len() <= properties.len());
        assert!(
            required
                .iter()
                .all(|name| properties.contains_key(name.as_str().unwrap()))
        );
    }
}
/** Shared discovery field copy explains native identities and stale-state refusals consistently. */
#[test]
fn discovery_fields_keep_designer_copy_and_native_limits() {
    let catalog = tools();
    let tool = catalog
        .iter()
        .find(|tool| tool["name"] == "page_select")
        .unwrap();
    let properties = &tool["inputSchema"]["properties"];
    for (field, description) in [
        (
            "tab_id",
            "Exact native tab ID returned by tab_list. No tab is selected automatically.",
        ),
        (
            "control_epoch",
            "The epoch returned by control_status for this tab after the human grant. A changed epoch is refused.",
        ),
        (
            "page_id",
            "Current page ID returned by page_inspect for this tab.",
        ),
        (
            "node_id",
            "Native node ID returned by page_inspect for this page.",
        ),
        (
            "expected_revision",
            "Observed page revision. A newer committed change causes refusal.",
        ),
        (
            "option_id",
            "Native option ID discovered for the targeted select; not its author value or label.",
        ),
    ] {
        assert_eq!(properties[field]["description"], description);
    }
    assert_eq!(properties["tab_id"]["maxLength"], ID_BYTES);
    assert_eq!(properties["tab_id"]["minLength"], 1);
    assert_eq!(properties["control_epoch"]["type"], "integer");
    assert_eq!(properties["control_epoch"]["minimum"], 0);
    assert_eq!(properties["expected_revision"]["maximum"], u64::MAX);
    for (name, field, bound) in [
        ("page_type", "text", TEXT_BYTES),
        ("page_navigate", "url", URL_BYTES),
    ] {
        assert_eq!(
            catalog.iter().find(|tool| tool["name"] == name).unwrap()["inputSchema"]["properties"]
                [field]["maxLength"],
            bound
        );
    }
    let scroll = catalog
        .iter()
        .find(|tool| tool["name"] == "page_scroll")
        .unwrap();
    assert_eq!(
        scroll["inputSchema"]["properties"]["y"]["description"],
        "Desired document scroll offset in logical points. The native runtime clamps it to the page."
    );
}
/** Neither dot spelling nor arbitrary underscore transformation opens an unreviewed native operation. */
#[test]
fn unknown_tools_cannot_select_native_or_human_only_operations() {
    for name in [
        "runtime.shutdown",
        "runtime_shutdown",
        "page.set_text",
        "page_set_text",
        "control_grant",
        "tab_select",
        "window_open",
        "page_execute",
        "page__navigate",
        "Page_inspect",
        "",
    ] {
        assert!(
            matches!(
                translate(name, &json!({}), "mcp-1"),
                Err(TranslationError::UnknownTool)
            ),
            "{name}"
        );
    }
}
