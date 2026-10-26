/** Markup scripts insert renders like page markup: its images load under the page budget, its
frames stay placeholders, inserted markup's scripts and handlers stay inert, and programmatic image handlers run. */
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

/** The public DOM projection includes author attributes that the accessibility inspection omits. */
fn attributes(runtime: &mut mars::runtime::Runtime, id: &str) -> Value {
    let nodes = agent_call(runtime, "page.nodes", json!({})).unwrap();
    nodes["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["attributes"]["id"] == id)
        .unwrap_or_else(|| panic!("no #{id} in {nodes}"))["attributes"]
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

/** Actual image decoding and its event task complete before the page's load event. */
#[test]
fn image_completion_precedes_window_load() {
    let (_fixture, _runtime, page) = load(
        "<p id=out></p><script>const i=new Image();const events=[];\
         i.onload=()=>events.push('image');i.src='/a.png';\
         window.onload=()=>{events.push('window:'+i.complete);\
         document.getElementById('out').textContent=events.join('|');};</script>",
    );
    scripts_ran(&page);
    assert_eq!(node(&page, "out")["text"], "image|window:true", "{page}");
}

/** Source selection before connection preserves the same image through parser-assigned identities. */
#[test]
fn source_before_append_loads_the_same_image() {
    let (_fixture, runtime, page) = load(
        "<p id=out></p><script>const i=new Image();i.id='pic';i.src='/a.png';\
         i.onload=()=>document.getElementById('out').textContent=i.isConnected+':'+i.naturalWidth;\
         document.body.append(i);</script>",
    );
    scripts_ran(&page);
    assert_eq!(node(&page, "out")["text"], "true:2", "{page}");
    assert_eq!(runtime.snapshot().images.len(), 1);
    assert_eq!(node(&page, "pic")["bounds"]["width"], 2.0);
}

/** A completed detached decode survives repeated adoption without fetching or firing load again. */
#[test]
fn completed_detached_pixels_survive_adoption_without_another_request() {
    let body = "<p id=out></p><script>const i=new Image();i.id='pic';let count=0;\
         i.onload=()=>{count++;document.body.append(i);i.remove();document.body.append(i);\
         document.getElementById('out').textContent=count+':'+i.naturalWidth+'x'+i.naturalHeight};\
         i.src='/a.png';</script>";
    let (fixture, counts) =
        Fixture::routes_counted(vec![("/", html("", body)), ("/a.png", image())]);
    let mut runtime = desktop(&fixture.url);
    let page = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    scripts_ran(&page);
    assert_eq!(node(&page, "out")["text"], "1:2x1", "{page}");
    assert_eq!(counts.lock().unwrap().get("/a.png"), Some(&1));
    assert_eq!(runtime.snapshot().images.len(), 1);
    assert_eq!(node(&page, "pic")["bounds"]["width"], 2.0);
}

/** An adopted image removed again in its load callback retains pixels privately without entering the snapshot. */
#[test]
fn completed_image_append_then_remove_keeps_pixels_private() {
    let body = "<p id=out></p><script>const i=new Image();i.id='pic';\
         i.onload=()=>{document.body.append(i);i.remove();\
         document.getElementById('out').textContent=i.isConnected+':'+i.naturalWidth};\
         i.src='/a.png';</script>";
    let (fixture, counts) =
        Fixture::routes_counted(vec![("/", html("", body)), ("/a.png", image())]);
    let mut runtime = desktop(&fixture.url);
    let page = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    scripts_ran(&page);
    assert_eq!(node(&page, "out")["text"], "false:2", "{page}");
    assert_eq!(counts.lock().unwrap().get("/a.png"), Some(&1));
    assert!(runtime.snapshot().images.is_empty());
}

/** A legacy fallback decode cannot pretend the selected srcset resource loaded successfully. */
#[test]
fn failed_selected_source_does_not_fire_a_fallback_load_event() {
    let (_fixture, runtime, page) = load(
        "<p id=out></p><img id=pic src=/a.png srcset='/missing.png 2x'>\
         <script>const i=document.getElementById('pic');\
         i.onload=()=>document.getElementById('out').textContent='wrong';\
         i.onerror=()=>document.getElementById('out').textContent='error:'+i.naturalWidth;</script>",
    );
    scripts_ran(&page);
    assert_eq!(node(&page, "out")["text"], "error:0", "{page}");
    assert!(runtime.snapshot().images.is_empty(), "{page}");
}

/** A managed foreground image can reveal an ordinary element's background from its load handler. */
#[test]
fn managed_image_can_reveal_an_ordinary_css_background() {
    let body = "<div id=host></div><script>const i=new Image();const h=document.getElementById('host');\
         i.onload=()=>h.style.backgroundImage='url(/bg.png)';h.append(i);i.src='/a.png';</script>";
    let (fixture, counts) =
        Fixture::routes_counted(vec![("/", html("", body)), ("/a.png", image())]);
    let mut runtime = desktop(&fixture.url);
    let page = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    scripts_ran(&page);
    assert!(
        attributes(&mut runtime, "host")["style"]
            .as_str()
            .unwrap_or_default()
            .contains("/bg.png")
    );
    let served = counts.lock().unwrap();
    assert!(
        served.get("/bg.png").copied().unwrap_or_default() > 0,
        "served routes: {served:?}"
    );
    assert_eq!(runtime.snapshot().images.len(), 1, "{page}");
}

/** An inserted image hidden by CSS stays outside the painted image assets even after loading. */
#[test]
fn hidden_inserted_image_stays_outside_painted_assets() {
    let (_fixture, runtime, page) = load(
        "<style>.off{display:none}</style><div id=host></div><script>document.getElementById('host')\
         .innerHTML='<div class=off><img src=/a.png></div>';</script>",
    );
    scripts_ran(&page);
    assert_eq!(runtime.snapshot().images.len(), 0, "{page}");
}

/** Hidden images genuinely decode and can reveal the same pixels from their load handler. */
#[test]
fn hidden_image_load_can_reveal_its_decoded_pixels() {
    let (_fixture, mut runtime, page) = load(
        "<style>.off{display:none}</style><div id=host></div><script>\
         const i=new Image();i.id='pic';i.className='off';i.onload=()=>{\
         i.className='';i.setAttribute('data-loaded',i.naturalWidth+'x'+i.naturalHeight);};\
         document.getElementById('host').appendChild(i);i.src='/a.png';</script>",
    );
    scripts_ran(&page);
    assert_eq!(runtime.snapshot().images.len(), 1, "{page}");
    assert_eq!(node(&page, "pic")["bounds"]["width"], 2.0);
    assert_eq!(attributes(&mut runtime, "pic")["data-loaded"], "2x1");
}

/** Inserted frames, video and forms keep the page running; frames are empty replaced boxes. */
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
    assert!(text.contains("\"tag\":\"iframe\""), "{page}");
    assert!(text.contains("cell"), "{page}");
}

/** Scripts and inline handlers from inserted markup remain inert under the existing parser contract. */
#[test]
fn inserted_scripts_and_handlers_never_run() {
    let (_fixture, _runtime, page) = load(
        "<div id=host></div><p id=out>safe</p><script>document.getElementById('host').innerHTML=\
         '<script>document.getElementById(\"out\").textContent=\"wrong-script\"</'+'script>\
         <img src=/missing.png onerror=\"document.getElementById(&quot;out&quot;).textContent+=&quot;:error&quot;\">';</script>",
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

/** A script's class change on an ancestor that gives an untouched element a background loads
that background, as a style recalc does in Blink (Wikipedia's table-of-contents icon appears
once its script swaps `client-nojs` for `client-js` on <html>). */
#[test]
fn background_revealed_by_an_ancestor_class_change_loads() {
    let (_fixture, runtime, page) = load(
        "<style>.icon{display:none;width:20px;height:20px}\
         .js .icon{display:block;background:url(/a.png)}</style>\
         <span id=icon class=icon></span>\
         <script>document.documentElement.className='js';</script>",
    );
    scripts_ran(&page);
    assert_eq!(runtime.snapshot().images.len(), 1, "{page}");
}
