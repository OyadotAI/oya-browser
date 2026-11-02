/** The native operations behind the CDP front door, run through the real Runtime and engines. */
#[allow(dead_code)]
mod support;
use serde_json::{Value, json};
use support::{agent_call, desktop, html};

/** A page with a script that defines globals, a form and a long body to scroll. */
const PAGE: &str = r#"<!doctype html><html><head><title>Lab</title><meta name="kind" content="fixture"></head><body>
<h1 id="title">Lab</h1>
<form action="/done"><label for="name">Name</label><input id="name" name="name" value="">
<input id="pw" type="password" value="hunter2-secret"><button id="go">Go</button></form>
<div id="tall" style="height: 3000px"></div><p id="deep">Deep</p>
<script>
  window.answer = 42;
  window.user = { name: "Ada", tags: ["a", "b"], nested: { ok: true } };
  window.later = () => new Promise(resolve => setTimeout(() => resolve("done"), 10));
  document.getElementById("go").addEventListener("click", () => { document.title = "Clicked"; });
</script></body></html>"#;

/** A runtime on a page served by a loopback fixture. */
fn runtime() -> (support::Fixture, mars::runtime::Runtime) {
    let fixture = support::Fixture::routes(vec![
        ("/", html("", PAGE)),
        ("/done", html("", "<title>Done</title><h1>Done</h1>")),
    ]);
    let mut runtime = desktop(&fixture.url);
    agent_call(
        &mut runtime,
        "page.navigate",
        json!({"url":format!("{}/", fixture.url)}),
    )
    .unwrap();
    (fixture, runtime)
}

/** One devtools request. */
fn devtools(runtime: &mut mars::runtime::Runtime, request: Value) -> Value {
    agent_call(runtime, "page.devtools", json!({"request":request})).unwrap()
}

/** Evaluation returns mirrors by value and by handle; handles answer getProperties. */
#[test]
fn evaluate_returns_mirrors_and_handles() {
    let (_fixture, mut runtime) = runtime();
    let sum = devtools(
        &mut runtime,
        json!({"kind":"evaluate","expression":"1 + 1","options":{"return_by_value":true}}),
    );
    assert_eq!(sum["result"]["type"], "number");
    assert_eq!(sum["result"]["value"], 2);
    let title = devtools(
        &mut runtime,
        json!({"kind":"evaluate","expression":"document.title","options":{"return_by_value":true}}),
    );
    assert_eq!(title["result"]["value"], "Lab");
    let user = devtools(
        &mut runtime,
        json!({"kind":"evaluate","expression":"user","options":{"return_by_value":true}}),
    );
    assert_eq!(
        user["result"]["value"],
        json!({"name":"Ada","tags":["a","b"],"nested":{"ok":true}})
    );
    let handle = devtools(
        &mut runtime,
        json!({"kind":"evaluate","expression":"user","options":{"preview":true}}),
    );
    assert_eq!(handle["result"]["type"], "object");
    assert_eq!(handle["result"]["className"], "Object");
    assert!(
        handle["result"]["preview"]["properties"]
            .as_array()
            .unwrap()
            .len()
            >= 3
    );
    let id: u32 = handle["result"]["objectId"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let properties = devtools(
        &mut runtime,
        json!({"kind":"get_properties","handle":id,"own_only":true}),
    );
    let names: Vec<&str> = properties["result"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["name", "tags", "nested"]);
    let called = devtools(
        &mut runtime,
        json!({"kind":"call_function","declaration":"function (suffix) { return this.name + suffix; }","this":id,"arguments":[{"value":"!"}],"options":{"return_by_value":true}}),
    );
    assert_eq!(called["result"]["value"], "Ada!");
    devtools(&mut runtime, json!({"kind":"release","handles":[id]}));
    let gone = agent_call(
        &mut runtime,
        "page.devtools",
        json!({"request":{"kind":"get_properties","handle":id}}),
    );
    assert!(gone.unwrap_err().message.contains("Could not find object"));
}

/** Exceptions come back as exception details; promises are awaited through page time. */
#[test]
fn exceptions_and_promises_follow_chrome() {
    let (_fixture, mut runtime) = runtime();
    let thrown = devtools(
        &mut runtime,
        json!({"kind":"evaluate","expression":"\nthrow new TypeError('nope')"}),
    );
    assert_eq!(thrown["exceptionDetails"]["text"], "Uncaught");
    assert_eq!(thrown["exceptionDetails"]["lineNumber"], 1);
    assert_eq!(thrown["exceptionDetails"]["exception"]["subtype"], "error");
    assert!(
        thrown["exceptionDetails"]["exception"]["description"]
            .as_str()
            .unwrap()
            .starts_with("TypeError: nope")
    );
    let awaited = devtools(
        &mut runtime,
        json!({"kind":"evaluate","expression":"later()","options":{"await_promise":true,"return_by_value":true}}),
    );
    assert_eq!(awaited["result"]["value"], "done", "{awaited}");
    let nan = devtools(
        &mut runtime,
        json!({"kind":"evaluate","expression":"NaN","options":{"return_by_value":true}}),
    );
    assert_eq!(nan["result"]["unserializableValue"], "NaN");
    let node = devtools(
        &mut runtime,
        json!({"kind":"evaluate","expression":"document.querySelector('#name')"}),
    );
    assert_eq!(node["result"]["subtype"], "node");
    assert_eq!(node["result"]["className"], "HTMLInputElement");
    assert_eq!(node["result"]["description"], "input#name");
    assert!(node["result"]["marsNode"].is_number());
}

/** Script changes through evaluation commit as a mutation, with the page's console captured. */
#[test]
fn evaluation_changes_commit_and_log() {
    let (_fixture, mut runtime) = runtime();
    let before = agent_call(&mut runtime, "page.summary", json!({})).unwrap();
    let changed = devtools(
        &mut runtime,
        json!({"kind":"evaluate","expression":"console.log('hi', 1); document.getElementById('title').textContent = 'Changed'; document.title"}),
    );
    assert_eq!(changed["console"][0]["text"], "hi 1");
    assert!(changed["revision"].as_u64().unwrap() > before["revision"].as_u64().unwrap());
    let summary = agent_call(&mut runtime, "page.summary", json!({})).unwrap();
    assert_eq!(summary["headings"][0]["text"], "Changed");
}

/** The DOM view lists nodes with attributes and boxes, never a password's value. */
#[test]
fn dom_view_hides_secrets_and_serializes() {
    let (_fixture, mut runtime) = runtime();
    let nodes = agent_call(&mut runtime, "page.nodes", json!({})).unwrap();
    let list = nodes["nodes"].as_array().unwrap();
    let password = list
        .iter()
        .find(|node| node["attributes"]["type"] == "password")
        .unwrap();
    assert!(password.get("value").is_none());
    assert!(password["attributes"].get("value").is_none());
    let title = list
        .iter()
        .find(|node| node["attributes"]["id"] == "title")
        .unwrap();
    assert_eq!(title["text"], "Lab");
    assert!(title["bounds"].is_array());
    assert_eq!(nodes["head"][1]["attributes"]["name"], "kind");
    let id = title["id"].as_u64().unwrap() as usize;
    let outer = agent_call(&mut runtime, "page.outer_html", json!({"node":id})).unwrap();
    assert_eq!(outer["outer_html"], "<h1 id=\"title\">Lab</h1>");
    let whole = agent_call(
        &mut runtime,
        "page.outer_html",
        json!({"node":nodes["virtual"]["html"]}),
    )
    .unwrap();
    let text = whole["outer_html"].as_str().unwrap();
    assert!(text.starts_with("<html><head><title>Lab</title>"));
    assert!(!text.contains("hunter2"));
    let revision = nodes["revision"].as_u64().unwrap();
    let set = agent_call(
        &mut runtime,
        "page.set_attribute",
        json!({"page_id":nodes["page_id"],"expected_revision":revision,"node":id,"name":"data-x","value":"1"}),
    )
    .unwrap();
    assert!(set["revision"].as_u64().unwrap() > revision);
    let deep = list
        .iter()
        .find(|node| node["attributes"]["id"] == "deep")
        .unwrap();
    let scrolled = agent_call(
        &mut runtime,
        "page.scroll_into_view",
        json!({"page_id":set["page_id"],"expected_revision":set["revision"],"node":deep["id"]}),
    )
    .unwrap();
    assert!(scrolled["scroll_y"].as_f64().unwrap() > 0.0, "{scrolled}");
}

/** Cookies set by the trusted client are listed with HttpOnly values and deleted by name. */
#[test]
fn cookies_round_trip() {
    let (fixture, mut runtime) = runtime();
    let set = agent_call(
        &mut runtime,
        "page.cookies_set",
        json!({"cookies":[{"name":"sid","value":"s3cret","url":format!("{}/", fixture.url),"http_only":true},{"name":"theme","value":"dark","url":format!("{}/", fixture.url)}]}),
    )
    .unwrap();
    assert_eq!(set["set"], 2);
    let listed = agent_call(&mut runtime, "page.cookies_list", json!({})).unwrap();
    let cookies = listed["cookies"].as_array().unwrap();
    assert_eq!(cookies.len(), 2);
    let sid = cookies
        .iter()
        .find(|cookie| cookie["name"] == "sid")
        .unwrap();
    assert_eq!(sid["value"], "s3cret");
    assert_eq!(sid["http_only"], true);
    let script = devtools(
        &mut runtime,
        json!({"kind":"evaluate","expression":"document.cookie","options":{"return_by_value":true}}),
    );
    assert_eq!(script["result"]["value"], "theme=dark");
    let host = url::Url::parse(&fixture.url).unwrap();
    agent_call(
        &mut runtime,
        "page.cookies_delete",
        json!({"name":"sid","domain":host.host_str().unwrap()}),
    )
    .unwrap();
    let listed = agent_call(&mut runtime, "page.cookies_list", json!({})).unwrap();
    assert_eq!(listed["cookies"].as_array().unwrap().len(), 1);
    agent_call(&mut runtime, "page.cookies_clear", json!({})).unwrap();
    let listed = agent_call(&mut runtime, "page.cookies_list", json!({})).unwrap();
    assert!(listed["cookies"].as_array().unwrap().is_empty());
}

/** Emulation changes the viewport and what scripts read; a peek leaves no trace. */
#[test]
fn emulation_and_peeks() {
    let (_fixture, mut runtime) = runtime();
    agent_call(
        &mut runtime,
        "page.emulate",
        json!({"viewport":{"width":800.0,"height":600.0},"user_agent":"Mars-Test/1.0","appearance":"dark"}),
    )
    .unwrap();
    let width = devtools(
        &mut runtime,
        json!({"kind":"evaluate","expression":"[innerWidth, navigator.userAgent, matchMedia('(prefers-color-scheme: dark)').matches]","options":{"return_by_value":true}}),
    );
    assert_eq!(width["result"]["value"][0], 800);
    assert_eq!(width["result"]["value"][1], "Mars-Test/1.0");
    let peek = agent_call(
        &mut runtime,
        "page.devtools_peek",
        json!({"request":{"kind":"evaluate","expression":"window.peeked = 1; document.title = 'Peeked'; 'seen'","options":{"return_by_value":true}}}),
    )
    .unwrap();
    assert_eq!(peek["result"]["value"], "seen");
    assert_eq!(peek["ephemeral"], true);
    let after = devtools(
        &mut runtime,
        json!({"kind":"evaluate","expression":"[typeof window.peeked, document.title]","options":{"return_by_value":true}}),
    );
    assert_eq!(after["result"]["value"], json!(["undefined", "Lab"]));
}
