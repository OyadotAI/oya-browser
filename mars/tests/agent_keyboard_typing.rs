/** Typed text, Backspace and fills reach page listeners as a browser fires them: each key's events
in order, the edit between beforeinput and input, and a cancelled keydown typing nothing. */
#[allow(dead_code)]
mod support;
use serde_json::{Value, json};
use support::{Fixture, agent_call, desktop, html};

/** A field whose page records every key and edit event, with the field's value at the moment
each one fires, so the order and the value-before-input rule are both observable. */
const LOG_PAGE: &str = "<!doctype html><input id=q aria-label=Search><script>\
const q=document.getElementById('q');const log=[];window.log=log;\
for(const k of ['keydown','keypress','keyup','change'])document.addEventListener(k,e=>log.push(k+':'+(e.key||'')+':'+e.isTrusted+':'+(e.target===q)+':'+q.value));\
for(const k of ['beforeinput','input'])document.addEventListener(k,e=>log.push(k+':'+e.inputType+':'+e.data+':'+e.isTrusted+':'+(e.target===q)+':'+q.value));\
</script>";

/** The same field, whose keydown handler cancels the letter x. */
const CANCEL_PAGE: &str = "<!doctype html><input id=q aria-label=Search><script>\
const q=document.getElementById('q');const log=[];window.log=log;\
document.addEventListener('keydown',e=>{log.push('down:'+e.key);if(e.key==='x')e.preventDefault()});\
document.addEventListener('keyup',e=>log.push('up:'+e.key));\
document.addEventListener('input',e=>log.push('input:'+q.value));\
</script>";

/** The current page identity and revision, and the node id of the field with this html id. */
fn field(runtime: &mut mars::runtime::Runtime, html_id: &str) -> (Value, String) {
    let at = agent_call(runtime, "page.inspect", json!({})).unwrap();
    let node = at["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["html_id"] == html_id)
        .unwrap_or_else(|| panic!("no #{html_id}"))["node_id"]
        .as_str()
        .unwrap()
        .to_owned();
    (at, node)
}

/** Presses `params` into the field `html_id` at the current revision. */
fn press(runtime: &mut mars::runtime::Runtime, html_id: &str, params: Value) {
    let (at, node) = field(runtime, html_id);
    let mut request =
        json!({"page_id":at["page_id"],"expected_revision":at["revision"],"node_id":node});
    request
        .as_object_mut()
        .unwrap()
        .extend(params.as_object().unwrap().clone());
    agent_call(runtime, "page.press", request).unwrap();
}

/** The page's recorded events, one entry each, in the order they fired. */
fn log(runtime: &mut mars::runtime::Runtime) -> Vec<String> {
    let result = agent_call(
        runtime,
        "page.devtools",
        json!({"request":{"kind":"evaluate","expression":"JSON.stringify(window.log)"}}),
    )
    .unwrap();
    serde_json::from_str(result["result"]["value"].as_str().unwrap()).unwrap()
}

/** The field's current value, read by the page itself. */
fn value(runtime: &mut mars::runtime::Runtime) -> String {
    let result = agent_call(
        runtime,
        "page.devtools",
        json!({"request":{"kind":"evaluate","expression":"document.getElementById('q').value"}}),
    )
    .unwrap();
    result["result"]["value"].as_str().unwrap().to_owned()
}

/** Typed letters fire keydown, keypress, beforeinput, input and keyup per character, in that order, with the value updated before input fires. */
#[test]
fn typed_text_fires_each_characters_events_in_browser_order() {
    let site = Fixture::raw(html("", LOG_PAGE));
    let mut runtime = desktop(&site.url);
    agent_call(&mut runtime, "page.navigate", json!({"url":site.url})).unwrap();
    press(&mut runtime, "q", json!({"text":"ab"}));
    assert_eq!(
        log(&mut runtime),
        [
            "keydown:a:true:true:",
            "keypress:a:true:true:",
            "beforeinput:insertText:a:true:true:",
            "input:insertText:a:true:true:a",
            "keyup:a:true:true:a",
            "keydown:b:true:true:a",
            "keypress:b:true:true:a",
            "beforeinput:insertText:b:true:true:a",
            "input:insertText:b:true:true:ab",
            "keyup:b:true:true:ab",
        ]
    );
    assert_eq!(value(&mut runtime), "ab");
}

/** Backspace fires keydown, beforeinput (deleteContentBackward), input and keyup, with no keypress, and removes the last character. */
#[test]
fn backspace_fires_its_events_and_removes_the_last_character() {
    let site = Fixture::raw(html("", LOG_PAGE));
    let mut runtime = desktop(&site.url);
    agent_call(&mut runtime, "page.navigate", json!({"url":site.url})).unwrap();
    press(&mut runtime, "q", json!({"text":"ab"}));
    agent_call(
        &mut runtime,
        "page.devtools",
        json!({"request":{"kind":"evaluate","expression":"window.log.length=0"}}),
    )
    .unwrap();
    press(&mut runtime, "q", json!({"key":"Backspace"}));
    assert_eq!(
        log(&mut runtime),
        [
            "keydown:Backspace:true:true:ab",
            "beforeinput:deleteContentBackward:null:true:true:ab",
            "input:deleteContentBackward:null:true:true:a",
            "keyup:Backspace:true:true:a",
        ]
    );
    assert_eq!(value(&mut runtime), "a");
}

/** Enter fires keydown, keypress and keyup at the field and no edit events, as a browser does for a single-line field. */
#[test]
fn enter_fires_key_events_without_an_edit() {
    let site = Fixture::raw(html("", LOG_PAGE));
    let mut runtime = desktop(&site.url);
    agent_call(&mut runtime, "page.navigate", json!({"url":site.url})).unwrap();
    press(&mut runtime, "q", json!({"key":"Enter"}));
    assert_eq!(
        log(&mut runtime),
        [
            "keydown:Enter:true:true:",
            "keypress:Enter:true:true:",
            "keyup:Enter:true:true:",
        ]
    );
    assert_eq!(value(&mut runtime), "");
}

/** A cancelled keydown types nothing: no keypress, beforeinput or input, but keyup still fires, and the next key still types. */
#[test]
fn a_cancelled_keydown_types_nothing_and_keyup_still_fires() {
    let site = Fixture::raw(html("", CANCEL_PAGE));
    let mut runtime = desktop(&site.url);
    agent_call(&mut runtime, "page.navigate", json!({"url":site.url})).unwrap();
    press(&mut runtime, "q", json!({"text":"xy"}));
    assert_eq!(
        log(&mut runtime),
        ["down:x", "up:x", "down:y", "input:y", "up:y"]
    );
    assert_eq!(value(&mut runtime), "y");
}

/** A fill (page.type) replaces the whole value in one beforeinput and input, then change, and no key events. */
#[test]
fn a_fill_replaces_the_value_with_one_edit_and_no_key_events() {
    let site = Fixture::raw(html("", LOG_PAGE));
    let mut runtime = desktop(&site.url);
    agent_call(&mut runtime, "page.navigate", json!({"url":site.url})).unwrap();
    let (at, node) = field(&mut runtime, "q");
    agent_call(
        &mut runtime,
        "page.type",
        json!({"page_id":at["page_id"],"expected_revision":at["revision"],"node_id":node,"text":"hello"}),
    )
    .unwrap();
    assert_eq!(
        log(&mut runtime),
        [
            "beforeinput:insertText:hello:true:true:",
            "input:insertText:hello:true:true:hello",
            "change::true:true:hello",
        ]
    );
    assert_eq!(value(&mut runtime), "hello");
}

/** A field without scripts takes typed text natively: no listener runs, and the value is the typed text. */
#[test]
fn typing_without_scripts_edits_the_value_natively() {
    let site = Fixture::raw(html("", "<!doctype html><input id=q aria-label=Search>"));
    let mut runtime = desktop(&site.url);
    agent_call(&mut runtime, "page.navigate", json!({"url":site.url})).unwrap();
    press(&mut runtime, "q", json!({"text":"abc"}));
    press(&mut runtime, "q", json!({"key":"Backspace"}));
    assert_eq!(value(&mut runtime), "ab");
}

/** A secret field still fires each key's events, but its text never reaches the page's listeners. */
#[test]
fn a_secret_field_fires_key_events_without_its_text() {
    let page = "<!doctype html><input id=q type=password aria-label=Secret><script>\
const q=document.getElementById('q');const log=[];window.log=log;\
document.addEventListener('keydown',e=>log.push('down:'+e.key+':'+q.value));\
document.addEventListener('input',e=>log.push('input:'+q.value));\
</script>";
    let site = Fixture::raw(html("", page));
    let mut runtime = desktop(&site.url);
    agent_call(&mut runtime, "page.navigate", json!({"url":site.url})).unwrap();
    press(&mut runtime, "q", json!({"text":"hu"}));
    assert_eq!(
        log(&mut runtime),
        ["down:h:", "input:", "down:u:", "input:"]
    );
}
