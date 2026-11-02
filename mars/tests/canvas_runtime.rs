/* Real engine processes exercise drawing, export, publication and language shaping without external sites. */
#![cfg(target_os = "macos")]
#[allow(dead_code)]
mod support;
use serde_json::{Value, json};
use support::{Fixture, agent_call, desktop, html};

/** Inspectable text comes from the actual committed document. */
fn text<'a>(page: &'a Value, id: &str) -> &'a str {
    page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["html_id"] == id)
        .unwrap()["text"]
        .as_str()
        .unwrap()
}

#[test]
fn canvas_fixture_completes_through_native_runtime_and_agent_screenshot() {
    let fixture = Fixture::raw(html("", include_str!("../fixtures/canvas.html")));
    let mut runtime = desktop(&fixture.url);
    let page = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    let button = page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["html_id"] == "draw")
        .unwrap();
    agent_call(&mut runtime,"page.click",json!({"page_id":page["page_id"],"expected_revision":page["revision"],"node_id":button["node_id"]})).unwrap();
    let page = agent_call(&mut runtime, "page.inspect", json!({})).unwrap();
    assert!(text(&page, "status").starts_with("Checks completed."));
    assert!(text(&page, "results").contains("PNG decoded: 300 × 150"));
    assert!(text(&page, "results").contains("WebGPU adapter: null"));
    let started = std::time::Instant::now();
    for _ in 0..20 {
        let screenshot = agent_call(&mut runtime, "page.screenshot", json!({})).unwrap();
        assert_eq!(screenshot["mime"], "image/png");
        assert!(
            screenshot["png_base64"]
                .as_str()
                .unwrap()
                .starts_with("iVBORw0KGgo")
        );
    }
    println!(
        "headless screenshot workload: 20 screenshots, {:.2}ms wall; includes raster and PNG encoding",
        started.elapsed().as_secs_f64() * 1000.0
    );
}

#[test]
fn navigation_headers_and_javascript_share_configured_languages() {
    let source = "<p id='language'></p><script>document.getElementById('language').textContent=navigator.languages.join(',')</script>";
    for tags in [vec!["fr-CA".into(), "fr".into()], vec!["ja".into()]] {
        let fixture = Fixture::raw(html("", source));
        let mut runtime = desktop(&fixture.url).with_languages(tags.clone());
        let page = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
        assert_eq!(text(&page, "language"), tags.join(","));
        let request = fixture.last_request.lock().unwrap().to_ascii_lowercase();
        assert!(request.contains(&format!(
            "accept-language: {}\r\n",
            mars_core::languages::header(&tags).to_ascii_lowercase()
        )));
    }
}

/** PNG exports must preserve straight RGBA bytes, including partially transparent pixels. */
fn png_pixels(bytes: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut reader = png::Decoder::new(bytes).read_info().unwrap();
    let mut pixels = vec![0; reader.output_buffer_size()];
    let frame = reader.next_frame(&mut pixels).unwrap();
    assert_eq!((frame.width, frame.height), (width, height));
    assert_eq!(frame.color_type, png::ColorType::Rgba);
    pixels.truncate(frame.buffer_size());
    pixels
}

/** Exercise service transport, document commit, display publication and both PNG export paths. */
fn published_pixels(width: u32, height: u32, drawing: &str) -> Vec<u8> {
    let body = format!(
        "<canvas id='paint' width='{width}' height='{height}'></canvas><p id='pixels'></p><p id='png'></p><script>\
        const c=document.getElementById('paint'),a=c.getContext('2d');{drawing}\
        document.getElementById('pixels').textContent=JSON.stringify(Array.from(a.getImageData(0,0,c.width,c.height).data));\
        document.getElementById('png').textContent=JSON.stringify(Array.from(Uint8Array.fromBase64(c.toDataURL().split(',')[1])));</script>"
    );
    let fixture = Fixture::raw(html("", &body));
    let mut runtime = desktop(&fixture.url);
    let page = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    assert_published_pixels(&runtime.snapshot(), &page, width, height)
}

/** Native display content and JS readback must describe one committed backing allocation. */
fn assert_published_pixels(
    snapshot: &mars::runtime::BrowserSnapshot,
    page: &Value,
    width: u32,
    height: u32,
) -> Vec<u8> {
    let document = snapshot.document.as_ref().unwrap();
    let bitmap = document
        .nodes
        .iter()
        .find(|node| node.html_id == "paint")
        .unwrap()
        .canvas
        .as_ref()
        .unwrap();
    let readback: Vec<u8> = serde_json::from_str(text(page, "pixels")).unwrap();
    assert_eq!((bitmap.width, bitmap.height), (width, height));
    assert_eq!(readback, bitmap.pixels.as_ref());
    assert!(
        snapshot
            .display
            .as_ref()
            .unwrap()
            .items
            .iter()
            .any(|item| item.canvas.as_ref() == Some(bitmap))
    );
    assert_png_exports(page, bitmap);
    readback
}

/** Decode both the page's exported PNG and a fresh native export of committed pixels. */
fn assert_png_exports(page: &Value, bitmap: &mars_core::canvas::Bitmap) {
    use mars_core::canvas::CanvasHost;
    let script_png: Vec<u8> = serde_json::from_str(text(page, "png")).unwrap();
    let native_png = mars_eye::platform::NativeCanvas.png(bitmap).unwrap();
    for encoded in [script_png, native_png] {
        assert_eq!(
            png_pixels(&encoded, bitmap.width, bitmap.height),
            bitmap.pixels.as_ref()
        );
    }
}

/** A rotated fractional rectangle must publish geometry coverage multiplied by global alpha. */
#[test]
fn transformed_alpha_rectangle_agrees_across_js_native_display_and_png() {
    let pixels = published_pixels(
        3,
        2,
        "a.fillStyle='red';a.globalAlpha=.5;a.setTransform(0,1,-1,0,2.5,0);a.fillRect(0,0,2,1);",
    );
    assert_eq!(
        pixels,
        [
            0, 0, 0, 0, 255, 0, 0, 64, 255, 0, 0, 64, 0, 0, 0, 0, 255, 0, 0, 64, 255, 0, 0, 64
        ]
    );
}

/** ImageData insertion ignores drawing state; cropped scaled copies use it and retain a source snapshot. */
#[test]
fn image_data_and_transformed_canvas_blit_agree_across_js_native_display_and_png() {
    let pixels = published_pixels(
        6,
        2,
        "const s=document.createElement('canvas');s.width=2;s.height=1;\
        const b=s.getContext('2d');b.translate(20,20);b.globalAlpha=0;\
        b.putImageData(new ImageData(new Uint8ClampedArray([255,0,0,255,0,0,255,255]),2),0,0);\
        a.imageSmoothingEnabled=false;a.globalAlpha=.5;a.translate(0,1);a.drawImage(s,-1,0,3,1,0,0,6,1);\
        s.width=1;b.fillStyle='green';b.fillRect(0,0,1,1);\
        a.putImageData(new ImageData(new Uint8ClampedArray([7,8,9,10]),1),5,0);",
    );
    assert_eq!(
        pixels,
        [
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 7, 8, 9, 10, 0, 0, 0, 0, 0,
            0, 0, 0, 255, 0, 0, 128, 255, 0, 0, 128, 0, 0, 255, 128, 0, 0, 255, 128
        ]
    );
}

/** Native glyphs are transformed before clipping and receive global alpha exactly once. */
#[test]
fn transformed_text_agrees_across_js_native_display_and_png() {
    let pixels = published_pixels(
        16,
        16,
        "a.font='12px sans-serif';a.fillStyle='red';a.globalAlpha=.5;a.translate(22,0);a.fillText('H',-20,12);",
    );
    let reference = published_pixels(
        16,
        16,
        "a.font='12px sans-serif';a.fillStyle='red';a.globalAlpha=.5;a.fillText('H',2,12);",
    );
    assert_eq!(pixels, reference);
    assert!(pixels.chunks_exact(4).any(|pixel| pixel[3] == 128));
    assert!(pixels.chunks_exact(4).all(|pixel| pixel[3] <= 128));
}
