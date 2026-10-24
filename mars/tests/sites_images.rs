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
