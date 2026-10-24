/** Markup scripts insert renders like page markup: its images load under the page budget, its
frames stay placeholders and its scripts and handlers never run. */
#[allow(dead_code)]
mod support;
use serde_json::{Value, json};
use support::{Fixture, agent_call, desktop, html};

/** A valid 2x1 RGBA PNG, built here so no opaque bytes are checked in. */
fn png() -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, 2, 1);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&[255, 0, 0, 255, 0, 0, 255, 255])
        .unwrap();
    bytes
}

/** A 200 image response. */
fn image() -> Vec<u8> {
    let body = png();
    let mut response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    response.extend_from_slice(&body);
    response
}

/** Navigate a fixture page whose body is `body`, with one image at /a.png. */
fn load(body: &str) -> (Fixture, mars::runtime::Runtime, Value) {
    let page: &'static str = Box::leak(body.to_string().into_boxed_str());
    let fixture = Fixture::routes(vec![("/", html("", page)), ("/a.png", image())]);
    let mut runtime = desktop(&fixture.url);
    let page = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    (fixture, runtime, page)
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

/** Scripts on the page ran without being stopped. */
fn scripts_ran(page: &Value) {
    assert!(
        !page["degraded"]
            .as_array()
            .unwrap()
            .contains(&json!("scripts")),
        "{page}"
    );
}

/** An img inserted with innerHTML is fetched and laid out at its natural size. */
#[test]
fn inner_html_image_loads() {
    let (_fixture, runtime, page) = load(
        "<div id=host></div><script>document.getElementById('host').innerHTML=\
         '<img id=pic src=/a.png alt=pic>';</script>",
    );
    scripts_ran(&page);
    assert_eq!(runtime.snapshot().images.len(), 1, "{page}");
    assert_eq!(node(&page, "pic")["bounds"]["width"], 2.0);
}

/** An img built with createElement and given a src later loads too. */
#[test]
fn created_image_loads() {
    let (_fixture, runtime, page) = load(
        "<div id=host></div><script>const i=document.createElement('img');i.id='pic';\
         document.getElementById('host').appendChild(i);i.setAttribute('src','/a.png');</script>",
    );
    scripts_ran(&page);
    assert_eq!(runtime.snapshot().images.len(), 1, "{page}");
}

/** An inserted image hidden by the page's CSS is not fetched. */
#[test]
fn hidden_inserted_image_is_not_fetched() {
    let (_fixture, runtime, page) = load(
        "<style>.off{display:none}</style><div id=host></div><script>document.getElementById('host')\
         .innerHTML='<div class=off><img src=/a.png></div>';</script>",
    );
    scripts_ran(&page);
    assert_eq!(runtime.snapshot().images.len(), 0, "{page}");
}

/** Inserted frames, video and forms keep the page running; frames are placeholders. */
#[test]
fn inserted_frames_and_controls_do_not_stop_scripts() {
    let (_fixture, _runtime, page) = load(
        "<div id=host></div><p id=out>before</p><script>const h=document.getElementById('host');\
         h.insertAdjacentHTML('beforeend','<iframe src=https://ads.example/></iframe><video></video>\
         <form><input name=q><button>Go</button></form><table><tr><td>cell</td></tr></table>');\
         document.getElementById('out').textContent='after';</script>",
    );
    scripts_ran(&page);
    let text = page.to_string();
    assert!(text.contains("after"), "{page}");
    assert!(
        text.contains("Embedded frame (not supported yet)"),
        "{page}"
    );
    assert!(text.contains("cell"), "{page}");
}

/** Scripts and handlers in inserted markup never run. */
#[test]
fn inserted_scripts_and_handlers_never_run() {
    let (_fixture, _runtime, page) = load(
        "<div id=host></div><p id=out>safe</p><script>document.getElementById('host').innerHTML=\
         '<script>document.getElementById(\"out\").textContent=\"ran\"</'+'script>\
         <img src=/missing.png onerror=\"document.getElementById(&quot;out&quot;).textContent=&quot;ran&quot;\">';</script>",
    );
    scripts_ran(&page);
    assert_eq!(node(&page, "out")["text"], "safe", "{page}");
}

/** A frame built with createElement and appended keeps the page's scripts running. */
#[test]
fn created_frame_does_not_stop_scripts() {
    let (_fixture, _runtime, page) = load(
        "<p id=out>before</p><script>const f=document.createElement('iframe');f.src='https://ads.example/';\
         document.body.appendChild(f);document.getElementById('out').textContent='after';</script>",
    );
    scripts_ran(&page);
    assert_eq!(node(&page, "out")["text"], "after", "{page}");
}
