/** replaceChildren on an element whose text the parser kept on the element replaces that text,
so the old label is not painted beside the new one (Google's app launcher showed "MapsMaps"). */
#[allow(dead_code)]
mod support;
use serde_json::json;
use support::{Fixture, agent_call, desktop, html};

/** The span and text nodes `script` leaves, as tag:text. */
fn after(script: &str) -> Vec<String> {
    let body = format!("<span id=s>Maps</span><script>const s=document.getElementById('s');{script}</script>");
    let body: &'static str = Box::leak(body.into_boxed_str());
    let fixture = Fixture::routes(vec![("/", html("", body))]);
    let mut runtime = desktop(&fixture.url);
    agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    let nodes = agent_call(&mut runtime, "page.nodes", json!({})).unwrap();
    nodes["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|node| node["tag"] == "span" || node["tag"] == "#text")
        .map(|node| format!("{}:{}", node["tag"].as_str().unwrap(), node["text"].as_str().unwrap()))
        .collect()
}

/** The new text replaces the element's own text. */
#[test]
fn replace_children_replaces_the_elements_own_text() {
    assert_eq!(after("s.replaceChildren('Maps')"), ["span:", "#text:Maps"]);
}

/** With no arguments the element ends up empty. */
#[test]
fn replace_children_with_nothing_empties_the_element() {
    assert_eq!(after("s.replaceChildren()"), ["span:"]);
}
