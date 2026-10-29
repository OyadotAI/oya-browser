/** replaceChildren on an element whose text the parser kept on the element replaces that text,
so the old label is not painted beside the new one (Google's app launcher showed "MapsMaps"). */
#[allow(dead_code)]
mod support;
use serde_json::json;
use support::{Fixture, agent_call, desktop, html};

/** A framed page's original label is one text child before its scripts append new content. */
#[test]
fn framed_parser_text_has_one_dom_text_child() {
    let mut document = mars::document::parse("<span id=app>Maps</span>").unwrap();
    mars::document::normalize_document_text(&mut document).unwrap();
    let span = document
        .nodes
        .iter()
        .find(|node| node.html_id == "app")
        .unwrap();
    let children: Vec<_> = document
        .nodes
        .iter()
        .filter(|node| node.parent == Some(span.local_id))
        .collect();
    assert!(span.text.is_empty());
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].text, "Maps");
}

/** The span and text nodes `script` leaves, as tag:text. */
fn after(script: &str) -> Vec<String> {
    let body = format!(
        "<span id=s>Maps</span><script>const s=document.getElementById('s');{script}</script>"
    );
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
        .map(|node| {
            format!(
                "{}:{}",
                node["tag"].as_str().unwrap(),
                node["text"].as_str().unwrap()
            )
        })
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
