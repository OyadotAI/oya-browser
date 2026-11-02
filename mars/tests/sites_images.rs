/** Site images served by media type alone draw: LinkedIn's hero illustration is an SVG at an
address with no `.svg` extension, which Relay refuses as a bitmap. */
#[allow(dead_code)]
mod support;
use serde_json::{Value, json};
use support::{Fixture, agent_call, desktop, html};

/** A 200 response of `mime` carrying `body`. */
fn reply(mime: &str, body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

/** The inspected node with this author id. */
fn node(page: &Value, id: &str) -> Value {
    page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|value| value["html_id"] == id)
        .unwrap_or_else(|| panic!("no #{id} in {page}"))
        .clone()
}

/** Navigate a page holding one img whose source is `/hero`, served as `mime`. */
fn load(mime: &str, body: &'static str) -> Value {
    let fixture = Fixture::routes(vec![
        ("/", html("", "<img id=hero src=/hero alt=hero>")),
        ("/hero", reply(mime, body)),
    ]);
    let mut runtime = desktop(&fixture.url);
    agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap()
}

/** An extensionless SVG address draws at the drawing's intrinsic size, not as alt text. */
#[test]
fn extensionless_svg_image_draws() {
    let page = load(
        "image/svg+xml",
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"40\" height=\"20\"><path d=\"M0,0H40V20Z\"/></svg>",
    );
    let hero = node(&page, "hero");
    assert_eq!(hero["bounds"]["width"], 40.0, "{page}");
    assert_ne!(hero["text"], "hero", "{page}");
}

/** A response that is neither a bitmap nor an SVG still falls back to the alt text. */
#[test]
fn unsupported_image_type_shows_alt_text() {
    let page = load("text/plain", "not an image");
    assert_eq!(node(&page, "hero")["text"], "hero", "{page}");
}

/** Each `.svg` image page's script reads its own load event: size as the loaded image reports it. */
const LOAD_SIZES: &str = "<p id=out></p><script>for(const id of ['hero','two']){const i=document.getElementById(id);\
const o=document.getElementById('out');i.addEventListener('load',()=>{o.textContent+=id+' '+i.naturalWidth+'x'+i.naturalHeight+';'});\
i.addEventListener('error',()=>{o.textContent+=id+' error;'});}</script>";

/** A wordmark shaped like a site's: a viewBox with no width or height, a clip path, a `fill=none` root. */
const WORDMARK: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" fill=\"none\" viewBox=\"0 0 140 22\"><defs><clipPath id=\"a\"><path d=\"M0 0h140v22H0z\"/></clipPath></defs><g clip-path=\"url(#a)\"><path fill=\"#0e65c0\" d=\"M0 0h140v22H0z\"/></g></svg>";

/** A square logo with width, height and a viewBox. */
const LOGO: &str = "<svg height=\"18\" viewBox=\"4 4 188 188\" width=\"18\" xmlns=\"http://www.w3.org/2000/svg\"><path d=\"m4 4h188v188h-188z\" fill=\"#f60\"/></svg>";

/** Two `.svg` images on one page, each fetched once and each reporting its natural size to scripts. */
#[test]
fn svg_images_report_natural_size_and_are_fetched_once() {
    let body = format!(
        "<img id=hero src=/wordmark.svg alt=hero><img id=two src=/y.svg alt=two>{LOAD_SIZES}"
    );
    let (fixture, counts) = Fixture::routes_counted(vec![
        ("/", html("", &body)),
        ("/wordmark.svg", reply("image/svg+xml", WORDMARK)),
        ("/y.svg", reply("image/svg+xml", LOGO)),
    ]);
    let mut runtime = desktop(&fixture.url);
    let page = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    let text = texts(&page);
    assert!(text.contains("hero 140x22;"), "{page}");
    assert!(text.contains("two 18x18;"), "{page}");
    // A drawing with a viewBox but no width or height is laid out as the default object size, 300 wide.
    assert_eq!(node(&page, "hero")["bounds"]["width"], 300.0, "{page}");
    assert_eq!(node(&page, "two")["bounds"]["width"], 18.0, "{page}");
    let counts = counts.lock().unwrap();
    assert_eq!(counts.get("/wordmark.svg"), Some(&1), "{counts:?}");
    assert_eq!(counts.get("/y.svg"), Some(&1), "{counts:?}");
}

/** Text of the inspected nodes, joined, so assertions read what a person sees. */
fn texts(page: &Value) -> String {
    page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|node| node["text"].as_str())
        .collect::<Vec<_>>()
        .join(" ")
}
