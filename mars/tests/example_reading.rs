/** The unchanged example.com page and its frozen script commit through the real supervised services. */
#[allow(dead_code)]
mod support;
use mars::{
    protocol::Request,
    runtime::{
        EnginePrograms, Runtime,
        authority::{Authority, Owner, Permit},
    },
};
use serde_json::{Value, json};
use std::{
    path::Path,
    sync::{Arc, atomic::AtomicBool},
};
use support::Fixture;

/** The frozen page and script, byte-for-byte as example.com served them. */
const PAGE: &str = include_str!("../fixtures/reading/example.html");
const SCRIPT: &str = include_str!("../fixtures/reading/example-s.js");
/** The destination the script's inserted link must keep. */
const LEARN_MORE: &str = "https://iana.org/help/example-domains";

/** One raw HTTP response with the observed live headers. */
fn response(mime: &str, body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

/** Serves `/` and `/s.js` from one granted loopback origin. */
fn fixture() -> Fixture {
    Fixture::routes(vec![
        ("/", response("text/html; charset=utf-8", PAGE)),
        ("/s.js", response("text/javascript; charset=utf-8", SCRIPT)),
    ])
}

/** A desktop runtime over the owned engines with a fixed language list. */
fn runtime(origin: &str, languages: &[&str]) -> Runtime {
    Runtime::desktop(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![origin.into()],
        Authority::shared(Owner::Agent),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap()
    .with_languages(languages.iter().map(|tag| (*tag).into()).collect())
}

/** Agent calls carry the real tab owner and epoch. */
fn call(runtime: &mut Runtime, op: &str, mut params: Value) -> Value {
    if op.starts_with("page.") {
        params["tab_id"] = json!("tab-1");
    }
    if mars::runtime::mutating(op) {
        params["control_epoch"] = json!(1);
    }
    runtime
        .handle_as(
            Request::new("example-reading", op, params),
            Permit {
                owner: Owner::Agent,
                epoch: 1,
                tab_id: Some(1),
            },
        )
        .unwrap()
}

/** Navigates the fixture with `languages` and returns the committed page inspection. */
fn navigate(languages: &[&str]) -> (Runtime, Value, Fixture) {
    let fixture = fixture();
    let mut runtime = runtime(&fixture.url, languages);
    let url = format!("{}/", fixture.url);
    let page = call(&mut runtime, "page.navigate", json!({ "url": url }));
    (runtime, page, fixture)
}

/** Inspected nodes with this tag, in document order. */
fn tagged<'a>(page: &'a Value, tag: &str) -> Vec<&'a Value> {
    page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|node| node["tag"] == tag)
        .collect()
}

/** The script picks the first paragraph whose lang matches a preferred language, else English. */
#[test]
fn script_orders_paragraphs_by_navigator_languages() {
    for (languages, first) in [(["en-US"], "en"), (["fr-FR"], "fr"), (["xx"], "en")] {
        let (_runtime, page, _fixture) = navigate(&languages);
        let paragraphs = tagged(&page, "p");
        assert_eq!(paragraphs.len(), 6, "{languages:?}");
        assert_eq!(paragraphs[0]["lang"], first, "{languages:?}");
        let langs: Vec<_> = paragraphs.iter().map(|p| p["lang"].clone()).collect();
        for lang in ["en", "ar", "zh", "fr", "ru", "es"] {
            assert!(langs.contains(&json!(lang)), "{lang} missing: {langs:?}");
        }
    }
}

/** The inserted link keeps its destination and stays an actionable agent target. */
#[test]
fn learn_more_link_is_actionable() {
    let (_runtime, page, _fixture) = navigate(&["en-US"]);
    let links = tagged(&page, "a");
    assert_eq!(links.len(), 1);
    assert_eq!(links[0]["href"], LEARN_MORE);
    assert!(links[0]["text"].as_str().unwrap().contains("Learn more"));
}

/** The link paints only through its text, yet it has geometry and visible_only finds it: an
inline element's box is the union of what its content painted. */
#[test]
fn learn_more_link_is_found_on_screen() {
    let (mut runtime, page, _fixture) = navigate(&["en-US"]);
    let link = tagged(&page, "a")[0];
    assert!(link["bounds"]["width"].as_f64().unwrap() > 0.0, "{link}");
    let top = link["bounds"]["y"].as_f64().unwrap();
    let scrolled = call(
        &mut runtime,
        "page.scroll",
        json!({"page_id":page["page_id"],"expected_revision":page["revision"],"y":top}),
    );
    assert_eq!(tagged(&scrolled, "a")[0]["visible"], true, "{scrolled}");
    let found = call(
        &mut runtime,
        "page.find",
        json!({"role":"link","visible_only":true}),
    );
    assert_eq!(found["matches"][0]["name"], "Learn more", "{found}");
}

/** With a window showing the first paint, the settled page is laid out again after its script
built the link, so the link has a box rather than keeping the pre-script layout. */
#[test]
fn first_paint_layout_is_replaced_after_scripts_build_content() {
    let fixture = fixture();
    let mut runtime = runtime(&fixture.url, &["en-US"]);
    runtime.show_first_paint(Box::new(|_| {}));
    let page = call(
        &mut runtime,
        "page.navigate",
        json!({ "url": format!("{}/", fixture.url) }),
    );
    let link = tagged(&page, "a")[0];
    assert!(link["bounds"]["width"].as_f64().unwrap() > 0.0, "{link}");
}

/** The decorative SVG and its parts are never agent targets. */
#[test]
fn svg_icon_is_not_an_agent_target() {
    let (_runtime, page, _fixture) = navigate(&["en-US"]);
    for tag in ["svg", "g", "path", "use"] {
        assert!(tagged(&page, tag).is_empty(), "{tag} was inspectable");
    }
}

/** The inserted style applies: later paragraphs shrink and fade, and the icon is 44x44 at 0.55. */
#[test]
fn inserted_style_and_icon_paint() {
    let (mut runtime, page, _fixture) = navigate(&["en-US"]);
    let layout = call(&mut runtime, "page.layout", json!({}));
    let items = layout["items"].as_array().unwrap();
    let item = |node: &Value| {
        items
            .iter()
            .find(|item| item["node_id"] == node["node_id"])
            .unwrap_or_else(|| panic!("no layout item for {node}"))
    };
    let paragraphs = tagged(&page, "p");
    let first = item(paragraphs[0]);
    assert!(first.get("opacity").is_none_or(|opacity| opacity == 1.0));
    for paragraph in &paragraphs[1..] {
        assert_eq!(item(paragraph)["opacity"], 0.6);
    }
    let icon = items
        .iter()
        .find(|item| item["role"] == "image" && item.get("vector").is_some())
        .expect("painted svg icon");
    assert_eq!(icon["opacity"], 0.55);
    assert_eq!(icon["width"], 44.0);
    assert_eq!(icon["height"], 44.0);
}
