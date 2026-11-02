/**
 * Native operations behind the CDP front door. Each one is an ordinary agent operation: tab
 * scoped, admitted by the same permits, epochs and revision checks as clicks and typing, and
 * answering with Mars's own shapes. `crates/mdp` translates them into Chrome DevTools Protocol
 * methods; nothing here speaks CDP, holds a debugger or bypasses the runtime's ownership rules.
 */
mod cookies;
mod dom;
mod emulate;
mod evaluate;
mod history;
mod intercept;
pub(super) use intercept::intercept;
use serde_json::{Value, json};

/** Capabilities list the DevTools operations beside the other agent operations. */
pub(super) fn describe(result: &mut Value) {
    for (operation, shape) in [
        (
            "page.devtools",
            json!({"control_epoch":"u64","request":"{kind: evaluate|call_function|get_properties|await|release|release_group|resolve_node|node_of, ...}","commits":"changes the request made, like a click"}),
        ),
        (
            "page.devtools_peek",
            json!({"request":"as page.devtools","commits":"nothing: a read of the page's realm; handles die with the call"}),
        ),
        ("page.nodes", json!({})),
        ("page.history", json!({})),
        ("page.outer_html", json!({"node":"usize"})),
        (
            "page.set_attribute",
            json!({"control_epoch":"u64","page_id":"string","expected_revision":"u64","node":"usize","name":"string","value":"optional string (absent removes)"}),
        ),
        (
            "page.scroll_into_view",
            json!({"control_epoch":"u64","page_id":"string","expected_revision":"u64","node":"usize"}),
        ),
        ("page.cookies_list", json!({"urls":"optional [string]"})),
        (
            "page.cookies_set",
            json!({"control_epoch":"u64","cookies":"[{name, value, url?, domain?, path?, secure?, http_only?, same_site?, expires?}]"}),
        ),
        (
            "page.cookies_delete",
            json!({"control_epoch":"u64","name":"string","url":"optional string","domain":"optional string","path":"optional string"}),
        ),
        ("page.cookies_clear", json!({"control_epoch":"u64"})),
        (
            "page.emulate",
            json!({"control_epoch":"u64","viewport":"optional {width, height}","appearance":"optional light|dark","user_agent":"optional string","platform":"optional string","timezone_offset_minutes":"optional number","languages":"optional [string]"}),
        ),
        (
            "page.navigate_intercepted",
            json!({"control_epoch":"u64","url":"string","continue_url":"optional string","fulfill":"optional {status, headers, body_base64}","fail":"optional net error reason"}),
        ),
    ] {
        result["operations"][operation] = shape;
        result["operations"][operation]["tab_id"] = json!("string");
    }
}
