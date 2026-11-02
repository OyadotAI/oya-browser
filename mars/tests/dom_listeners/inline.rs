/** Real services run inline handler arguments in order and keep effects made before a throw. */
use super::*;

/** Loads the inline-argument page with these nodes, handler and remove() action. */
fn inline_page(nodes: &str, action: &str) -> (Fixture, Runtime, Value) {
    let body = format!(
        "{nodes}<p id=out>initial</p><button id=button type=button onclick=\"receive(remove(),document.getElementById('name').value)\">Bad</button><button id=probe type=button onclick='probe()'>Probe</button><script>var count=0;function remove(){{count++;window.localStorage.setItem('count',count);{action}}}function receive(a,b){{document.getElementById('out').textContent=b;}}function probe(){{document.getElementById('out').textContent=count+' '+window.localStorage.getItem('count');}}</script>"
    );
    let fixture = fixture(&body);
    let mut runtime = runtime(&fixture.url);
    let page = call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    (fixture, runtime, page)
}

/** An argument that throws after an earlier argument's effects keeps those effects, as browsers do. */
#[test]
fn inline_argument_failure_keeps_earlier_argument_effects() {
    let (_fixture, mut runtime, page) = inline_page(
        "<div id=parent><input id=name value=Ada></div>",
        "document.getElementById('parent').textContent='removed';",
    );
    let clicked = call(&mut runtime, "page.click", target(&page, "button")).unwrap();
    assert_eq!(text(&clicked, "parent"), "removed");
    assert_eq!(text(&clicked, "out"), "initial");
    let probed = call(&mut runtime, "page.click", target(&clicked, "probe")).unwrap();
    assert_eq!(text(&probed, "out"), "1 1");
}

/** A duplicate id reads the first match, whose missing value property is undefined and writes as empty text. */
#[test]
fn inline_argument_reads_the_first_duplicate_match() {
    let (_fixture, mut runtime, page) =
        inline_page("<p id=name>first</p><input id=name value=Ada>", "");
    let clicked = call(&mut runtime, "page.click", target(&page, "button")).unwrap();
    assert_eq!(text(&clicked, "out"), "");
    let probed = call(&mut runtime, "page.click", target(&clicked, "probe")).unwrap();
    assert_eq!(text(&probed, "out"), "1 1");
}

/** Native editing supplies current values to an ordinary inline argument without changing its handler contract. */
#[test]
fn inline_input_arguments_use_current_native_edits() {
    let body = "<input id=name value=Ada><p id=out>initial</p><button id=button type=button onclick=\"receive(document.getElementById('name').value)\">Read</button><script>function receive(value){document.getElementById('out').textContent=value;}</script>";
    let fixture = fixture(body);
    let mut runtime = runtime(&fixture.url);
    let mut page = call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    for value in ["Lin", "Rust 🐙", ""] {
        let mut params = target(&page, "name");
        params["text"] = json!(value);
        call(&mut runtime, "page.type", params).unwrap();
        page = call(&mut runtime, "page.inspect", json!({})).unwrap();
        page = call(&mut runtime, "page.click", target(&page, "button")).unwrap();
        assert_eq!(text(&page, "out"), value);
    }
}
