/** Hermetic native DOM journey integration tests retain real service boundaries. */
use super::*;

/** Cached references survive subprocess boundaries and observe current inputs without replaying initializers. */
#[test]
fn registered_bound_controller_reads_current_inputs_and_reload_resets_state() {
    let body = "<input id=name><p id=out>initial</p><button id=button type=button>Greet</button><script>var saved=make();function make(){const input=document.getElementById('name');const out=document.getElementById('out');return function(){this.count++;out.textContent=(input.value||'world')+' '+this.count;}.bind({count:0});}document.getElementById('button').addEventListener('click',()=>{saved.apply(null,[]);});</script>";
    let fixture = fixture(body);
    let mut runtime = runtime(&fixture.url);
    let mut page = call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    for (count, name) in [(1, "Ada"), (2, "Lin"), (3, "Rust 🐙"), (4, "")] {
        let mut params = target(&page, "name");
        params["text"] = json!(name);
        call(&mut runtime, "page.type", params).unwrap();
        page = call(&mut runtime, "page.inspect", json!({})).unwrap();
        page = call(&mut runtime, "page.click", target(&page, "button")).unwrap();
        assert_eq!(
            text(&page, "out"),
            format!("{} {count}", if name.is_empty() { "world" } else { name })
        );
    }
    let stale = target(&page, "button");
    let previous_page = page["page_id"].clone();
    page = call(&mut runtime, "page.reload", json!({})).unwrap();
    assert_ne!(page["page_id"], previous_page);
    assert!(call(&mut runtime, "page.click", stale).is_err());
    assert_eq!(text(&page, "out"), "initial");
    page = call(&mut runtime, "page.click", target(&page, "button")).unwrap();
    assert_eq!(text(&page, "out"), "world 1");
}

/** A listener that throws keeps its earlier storage, captured count and text, and every listener stays registered. */
#[test]
fn throwing_listener_keeps_earlier_effects_and_registrations() {
    let body = "<p id=out>initial</p><button id=good type=button>Good</button><button id=bad type=button>Bad</button><button id=read type=button>Read</button><script>var count=0;const out=document.getElementById('out');function update(){count++;window.localStorage.setItem('count',count);out.textContent=count;}document.getElementById('good').addEventListener('click',update);document.getElementById('bad').addEventListener('click',()=>{update();document.getElementById('missing').textContent='fail';});document.getElementById('read').addEventListener('click',()=>{out.textContent=count+' '+window.localStorage.getItem('count');});</script>";
    let fixture = fixture(body);
    let mut runtime = runtime(&fixture.url);
    let page = call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    let page = call(&mut runtime, "page.click", target(&page, "good")).unwrap();
    assert_eq!(text(&page, "out"), "1");
    let inspected = call(&mut runtime, "page.click", target(&page, "bad")).unwrap();
    assert_eq!(text(&inspected, "out"), "2");
    let inspected = call(&mut runtime, "page.click", target(&inspected, "read")).unwrap();
    assert_eq!(text(&inspected, "out"), "2 2");
    let inspected = call(&mut runtime, "page.click", target(&inspected, "good")).unwrap();
    assert_eq!(text(&inspected, "out"), "3");
}

/** Inspection never executes registered callbacks and an old revision cannot trigger another dispatch. */
#[test]
fn inspection_is_inert_and_stale_revision_clicks_do_not_execute_callbacks() {
    let body = "<p id=out>initial</p><button id=button type=button>Click</button><script>let count=0;document.getElementById('button').addEventListener('click',()=>{count++;document.getElementById('out').textContent=count;});</script>";
    let fixture = fixture(body);
    let mut runtime = runtime(&fixture.url);
    let page = call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    let stale = target(&page, "button");
    for _ in 0..3 {
        assert_eq!(
            text(
                &call(&mut runtime, "page.inspect", json!({})).unwrap(),
                "out"
            ),
            "initial"
        );
    }
    let page = call(&mut runtime, "page.click", stale.clone()).unwrap();
    assert_eq!(text(&page, "out"), "1");
    assert!(call(&mut runtime, "page.click", stale).is_err());
    let page = call(&mut runtime, "page.inspect", json!({})).unwrap();
    assert_eq!(text(&page, "out"), "1");
}

/** Listener support does not replace the native submit-button action or form serialization. */
#[test]
fn ordinary_submit_buttons_keep_native_form_navigation() {
    let body = "<form action='/submitted'><input name=name value=Ada><button id=submit>Submit</button></form>";
    let fixture = fixture(body);
    let mut runtime = runtime(&fixture.url);
    let page = call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    let submitted = call(&mut runtime, "page.click", target(&page, "submit")).unwrap();
    assert!(submitted["url"].as_str().unwrap().contains("/submitted"));
    assert!(
        fixture
            .last_request
            .lock()
            .unwrap()
            .starts_with("GET /submitted?name=Ada HTTP/1.1")
    );
}
