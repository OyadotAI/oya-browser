/** Scrolling through the real Runtime: a wheel over an app shell's inner scroller moves only that
scroller's contents (LinkedIn's sign-in variant scrolls its `<main>`, never the document), and a
viewport scroll reaches the page's scripts as a `scroll` event with `scrollY` set. */
#[allow(dead_code)]
mod support;
use serde_json::{Value, json};
use support::{Fixture, agent_call, desktop, html};

/** A viewport-high shell whose `<main>` is the only scroller, over 2000px of content. */
const SHELL: &str = "<!doctype html><title>Shell</title><style>html,body{margin:0;height:100%}\
    body{overflow:hidden}header{display:block;height:50px}main{display:block;height:400px;\
    overflow-x:hidden;overflow-y:scroll}section{display:block;height:2000px}</style>\
    <header>top</header><main><section id=s>content</section></main>";

/** The painted box of the author id `id` in a page.layout result. */
fn painted_y(layout: &Value, inspect: &Value, id: &str) -> f64 {
    let node = inspect["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["html_id"] == id)
        .unwrap()["node_id"]
        .clone();
    layout["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["node_id"] == node)
        .unwrap()["y"]
        .as_f64()
        .unwrap()
}

/** A wheel over the inner scroller moves its contents and leaves the document where it was. */
#[test]
fn a_wheel_over_an_inner_scroller_scrolls_it_not_the_document() {
    let fixture = Fixture::routes(vec![("/", html("", SHELL))]);
    let mut runtime = desktop(&fixture.url);
    let page = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    let before = agent_call(&mut runtime, "page.layout", json!({})).unwrap();
    assert_eq!(painted_y(&before, &page, "s"), 50.0);
    let wheel = agent_call(
        &mut runtime,
        "page.scroll_at",
        json!({"page_id":page["page_id"],"expected_revision":page["revision"],"x":100.0,"y":200.0,"delta_y":300.0}),
    )
    .unwrap();
    assert_eq!(wheel["viewport"]["scroll_y"], 0.0);
    let after = agent_call(&mut runtime, "page.layout", json!({})).unwrap();
    assert_eq!(painted_y(&after, &page, "s"), 50.0 - 300.0);
    // Past the end the scroller stays at its last offset (2000 - 400) and the wheel moves nothing.
    agent_call(
        &mut runtime,
        "page.scroll_at",
        json!({"page_id":page["page_id"],"expected_revision":page["revision"],"x":100.0,"y":200.0,"delta_y":5000.0}),
    )
    .unwrap();
    let end = agent_call(&mut runtime, "page.layout", json!({})).unwrap();
    assert_eq!(painted_y(&end, &page, "s"), 50.0 - 1600.0);
}

/** Scrolling the document fires `scroll` on the page's next background run, with scrollY set,
as lazy loaders need. */
#[test]
fn a_document_scroll_reaches_page_scripts() {
    let page = "<!doctype html><title>Tall</title><style>p{height:3000px;margin:0}</style><p>tall</p>\
        <script>addEventListener('scroll', () => { document.title = 'at ' + scrollY }, true)</script>";
    let fixture = Fixture::routes(vec![("/", html("", page))]);
    let mut runtime = desktop(&fixture.url);
    let page = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    let scrolled = agent_call(
        &mut runtime,
        "page.scroll",
        json!({"page_id":page["page_id"],"expected_revision":page["revision"],"y":700.0}),
    )
    .unwrap();
    assert_eq!(scrolled["viewport"]["scroll_y"], 700.0);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let mut title = String::new();
    while std::time::Instant::now() < deadline && title != "at 700" {
        runtime.run_page_scripts();
        std::thread::sleep(std::time::Duration::from_millis(20));
        let inspected = agent_call(&mut runtime, "page.inspect", json!({})).unwrap();
        title = inspected["title"].as_str().unwrap_or_default().to_owned();
    }
    assert_eq!(title, "at 700");
}

/** A script that measures a node it just inserted under a scrolled inner container sees the box
where it is painted: CSSOM View rects subtract every ancestor scroller's offset, so a lazy
loader on a page that scrolls its `<main>` sees rows enter the viewport. */
#[test]
fn script_geometry_subtracts_an_inner_scrollers_offset() {
    let shell = SHELL.replace(
        "</main>",
        "</main><script>addEventListener('scroll', () => { const d = document.createElement('div'); \
         d.textContent = 'row'; const s = document.getElementById('s'); s.insertBefore(d, s.firstChild); \
         d.getBoundingClientRect(); document.title = 'top ' + s.getBoundingClientRect().top }, true)</script>",
    );
    let fixture = Fixture::routes(vec![("/", html("", &shell))]);
    let mut runtime = desktop(&fixture.url);
    let page = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    agent_call(
        &mut runtime,
        "page.scroll_at",
        json!({"page_id":page["page_id"],"expected_revision":page["revision"],"x":100.0,"y":200.0,"delta_y":300.0}),
    )
    .unwrap();
    // The document cannot scroll; this only makes the page's scripts run their scroll listener.
    agent_call(
        &mut runtime,
        "page.scroll",
        json!({"page_id":page["page_id"],"expected_revision":page["revision"],"y":0.0}),
    )
    .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let mut title = String::new();
    while std::time::Instant::now() < deadline && !title.starts_with("top") {
        runtime.run_page_scripts();
        std::thread::sleep(std::time::Duration::from_millis(20));
        let inspected = agent_call(&mut runtime, "page.inspect", json!({})).unwrap();
        title = inspected["title"].as_str().unwrap_or_default().to_owned();
    }
    assert_eq!(title, "top -250");
}
