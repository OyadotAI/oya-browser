/*! Governed acquisition, native decoding and V9 canvas provenance stay connected across services. */
#![cfg(target_os = "macos")]
#[allow(dead_code)]
mod support;
use serde_json::{Value, json};
use std::sync::{Arc, atomic::AtomicBool};
use support::{Fixture, agent_call, html};

/** A tiny encoded fixture makes physical pixels observable without external image files. */
fn png() -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 2, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&[255, 0, 0, 255, 0, 255, 0, 255])
            .unwrap();
    }
    bytes
}

/** Exact binary response bytes pass through the same image fetch path as public pages. */
fn image_response() -> Vec<u8> {
    let bytes = png();
    let mut response = format!("HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", bytes.len()).into_bytes();
    response.extend(bytes);
    response
}

/** A real load event draws its source before reporting readback or the security exception. */
fn page(source: &str) -> Vec<u8> {
    html(
        "",
        &format!(
            "<p id='result'>pending</p><canvas id='c' width='2' height='1'></canvas><script>const i=new Image();i.onload=()=>{{const x=document.getElementById('c').getContext('2d');try{{x.drawImage(i,0,0);document.getElementById('result').textContent=Array.from(x.getImageData(0,0,2,1).data).join()}}catch(e){{document.getElementById('result').textContent=e.name}}}};i.onerror=()=>{{document.getElementById('result').textContent='error'}};i.src='{source}';</script>"
        ),
    )
}

/** Each fixture origin is explicitly authorized; tests never access the public network. */
fn runtime(origins: Vec<String>) -> mars::runtime::Runtime {
    use mars::runtime::{
        EnginePrograms, Runtime,
        authority::{Authority, Owner},
    };
    Runtime::desktop(
        EnginePrograms::same(std::path::Path::new(env!("CARGO_BIN_EXE_mars"))),
        origins,
        Authority::shared(Owner::Agent),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap()
}

/** Inspect the actual committed page after image settlement, including the load callback. */
fn result(page: &Value) -> &str {
    page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["html_id"] == "result")
        .unwrap()["text"]
        .as_str()
        .unwrap()
}

/** Same-origin pixels retain both source colors through the native completion transport. */
#[test]
fn same_origin_png_can_be_drawn_and_read_back() {
    let fixture = Fixture::routes(vec![
        ("/", page("/image.png")),
        ("/image.png", image_response()),
    ]);
    let mut runtime = runtime(vec![fixture.url.clone()]);
    let loaded = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    assert_eq!(result(&loaded), "255,0,0,255,0,255,0,255");
}

/** A same-origin URL redirecting across origins must not grant readback of the final image. */
#[test]
fn cross_origin_redirect_taints_canvas_pixels() {
    let image = Fixture::raw(image_response());
    let redirect = format!("HTTP/1.1 302 Found\r\nLocation: {}/image.png\r\nContent-Length: 0\r\nConnection: close\r\n\r\n", image.url).into_bytes();
    let fixture = Fixture::routes(vec![("/", page("/image.png")), ("/image.png", redirect)]);
    let mut runtime = runtime(vec![fixture.url.clone(), image.url.clone()]);
    let loaded = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    assert_eq!(result(&loaded), "SecurityError");
    let snapshot = runtime.snapshot();
    let bitmap = snapshot
        .document
        .as_ref()
        .unwrap()
        .nodes
        .iter()
        .find(|node| node.html_id == "c")
        .unwrap()
        .canvas
        .as_ref()
        .unwrap();
    assert_eq!(bitmap.pixels.as_ref(), &[255, 0, 0, 255, 0, 255, 0, 255]);
}
