/** A script-built consent dialog (CNN's shape): its link styles and box sizes match
Chromium's. */
#[allow(dead_code)]
mod support;
use serde_json::{Value, json};
use support::{Fixture, agent_call, desktop, html};

/** The page's reset, then a dialog built the way the consent script builds it. */
const PAGE: &str = "<style>html{box-sizing:border-box}*,::after,::before{box-sizing:inherit}\
body{margin:0}</style><script>setTimeout(()=>{\
const d=document.body.appendChild(document.createElement('div'));d.id='dlg';\
Object.assign(d.style,{padding:'40px 48px',position:'fixed',display:'block',width:'700px'});\
const p=d.appendChild(document.createElement('p'));p.innerHTML='See <a id=terms href=/t>Terms</a>.';\
Object.assign(p.firstElementChild.style,{textDecorationLine:'underline',\
textDecorationSkipInk:'none',textUnderlinePosition:'from-font'});\
const b=d.appendChild(document.createElement('a'));b.id='agree';b.textContent='Agree';\
Object.assign(b.style,{display:'flex',height:'48px',width:'135px',padding:'20px 32px'});\
document.title=p.firstElementChild.getAttribute('style');},300);</script>";

/** Navigate the dialog fixture: the runtime and the inspected page. */
fn load() -> (Fixture, Value) {
    let fixture = Fixture::routes(vec![("/", html("", PAGE))]);
    let mut runtime = desktop(&fixture.url);
    agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    for _ in 0..20 {
        std::thread::sleep(std::time::Duration::from_millis(50));
        runtime.run_page_scripts();
    }
    let page = agent_call(&mut runtime, "page.inspect", json!({})).unwrap();
    (fixture, page)
}

/** The inspected bounds (width, height) of the node with this author id. */
fn size(page: &Value, id: &str) -> (f64, f64) {
    let node = page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|value| value["html_id"] == id)
        .unwrap_or_else(|| panic!("no #{id} in {page}"));
    let bounds = &node["bounds"];
    (
        bounds["width"].as_f64().unwrap(),
        bounds["height"].as_f64().unwrap(),
    )
}

/** CSSOM camelCase writes of the text-decoration longhands land in the style attribute. */
#[test]
fn text_decoration_longhands_are_style_properties() {
    let (_fixture, page) = load();
    let style = page["title"].as_str().unwrap();
    assert!(style.contains("text-decoration-line: underline"), "{style}");
    assert!(
        style.contains("text-underline-position: from-font"),
        "{style}"
    );
}

/** `html{box-sizing:border-box} *{box-sizing:inherit}` makes descendants border-box. */
#[test]
fn root_box_sizing_is_inherited() {
    let (_fixture, page) = load();
    assert_eq!(size(&page, "dlg").0, 700.0);
    assert_eq!(size(&page, "agree"), (135.0, 48.0));
}
