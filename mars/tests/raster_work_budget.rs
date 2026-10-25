/** The windowless painter's shadow and vector work stays bounded: a huge box-shadow blur is
clipped to the canvas and charged to a per-screenshot budget, a repeated SVG image draws once per
size, and a drawing with tens of thousands of edges fills through an active edge table within a
budget, so a hostile page cannot stall or exhaust memory through page.screenshot. */
#[allow(dead_code)]
mod support;
use serde_json::json;
use std::time::{Duration, Instant};
use support::{Fixture, agent_call, desktop, html};

/** The screenshot bound, generous for debug builds on a loaded machine. */
const LIMIT: Duration = Duration::from_secs(2);
/** Copies of one SVG background on the repeated-image page. */
const COPIES: usize = 500;
/** Cubic segments in the many-edge drawing; each flattens into 16 edges. */
const CUBICS: usize = 2_000;
/** Cubics per path, keeping each `d` attribute well under the node value bound. */
const PER_PATH: usize = 1_000;
/** Distinct tile sizes the many-edge drawing is drawn at, and the shortest one's height. */
const SIZES: usize = 3;
const TALL: usize = 1_000;
/** The drawing's fill, which must show in the shot. */
const FILL: [u8; 3] = [0x00, 0x88, 0xff];

/** An SVG response with exact framing. */
fn svg(body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: image/svg+xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

/** A drawing of `cubics` low wavy cubic segments (a band a few rows tall), PER_PATH to a path laid side by side, each
 * filled down to the bottom edge, in a viewBox three units wide per cubic. */
fn wavy(cubics: usize) -> String {
    let paths: String = (0..cubics.div_ceil(PER_PATH))
        .map(|at| {
            let count = PER_PATH.min(cubics - at * PER_PATH);
            format!(
                "<path fill=\"#0088ff\" d=\"M{} 1000V500c1-9 2 9 3 0{}V1000Z\"/>",
                at * PER_PATH * 3,
                " 1-9 2 9 3 0".repeat(count - 1)
            )
        })
        .collect();
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} 1000\" \
         preserveAspectRatio=\"none\">{paths}</svg>",
        cubics * 3
    )
}

/** Standard padded base64 back to bytes. */
fn decode_base64(text: &str) -> Vec<u8> {
    let value = |c: u8| match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        _ => 63,
    };
    let digits: Vec<u8> = text.bytes().filter(|c| *c != b'=').collect();
    let mut out = Vec::new();
    for chunk in digits.chunks(4) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, c)| n | u32::from(value(*c)) << (18 - 6 * i));
        for i in 0..chunk.len() - 1 {
            out.push((n >> (16 - 8 * i)) as u8);
        }
    }
    out
}

/** How long one screenshot of `body` (with `/a.svg` serving `drawing`) takes after the page
 * has loaded, and its RGB pixels. */
fn shot_time(body: String, drawing: Option<String>) -> (Duration, Vec<u8>) {
    let page: &'static str = Box::leak(body.into_boxed_str());
    let mut routes = vec![("/", html("", page))];
    if let Some(drawing) = drawing {
        routes.push(("/a.svg", svg(&drawing)));
    }
    let fixture = Fixture::routes(routes);
    let mut runtime = desktop(&fixture.url);
    agent_call(
        &mut runtime,
        "page.emulate",
        json!({"viewport": {"width": 400.0, "height": 300.0}}),
    )
    .unwrap();
    agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    let started = Instant::now();
    let shot = agent_call(&mut runtime, "page.screenshot", json!({})).unwrap();
    let elapsed = started.elapsed();
    let png = decode_base64(shot["png_base64"].as_str().unwrap());
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png));
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().unwrap();
    let mut pixels = vec![0; reader.output_buffer_size()];
    let frame = reader.next_frame(&mut pixels).unwrap();
    pixels.truncate(frame.buffer_size());
    (elapsed, pixels)
}

/** Whether the drawing's fill shows anywhere in the shot. */
fn shows_fill(pixels: &[u8]) -> bool {
    pixels.chunks_exact(3).any(|pixel| pixel == FILL)
}

/** A box-shadow blurred by 100000px is clipped and budgeted instead of convolving a region
 * hundreds of thousands of pixels wide. */
#[test]
fn a_huge_shadow_blur_is_bounded() {
    let page = "<style>body{margin:0}div{margin:50px;width:100px;height:100px;\
                box-shadow:0 0 100000px red}</style><div></div>"
        .to_string();
    let (elapsed, _) = shot_time(page, None);
    assert!(elapsed < LIMIT, "screenshot took {elapsed:?}");
}

/** Hundreds of boxes showing one complex SVG at one size draw it once. */
#[test]
fn a_repeated_svg_image_draws_once() {
    let boxes = "<i></i>".repeat(COPIES);
    let page = format!(
        "<style>body{{margin:0}}i{{display:inline-block;width:20px;height:20px;\
         background:url(/a.svg) 0 100%/400px 300px}}</style>{boxes}"
    );
    let (elapsed, pixels) = shot_time(page, Some(wavy(CUBICS)));
    assert!(elapsed < LIMIT, "screenshot took {elapsed:?}");
    assert!(shows_fill(&pixels), "the drawing did not paint");
}

/** A drawing of tens of thousands of edges, drawn tall at several sizes, fills in bounded
 * time: each edge is scanned only on the rows it crosses. */
#[test]
fn a_many_edge_drawing_is_bounded() {
    let boxes: String = (0..SIZES)
        .map(|at| format!("<i style=\"background-size:400px {}px\"></i>", TALL + at))
        .collect();
    let page = format!(
        "<style>body{{margin:0}}i{{display:block;width:400px;height:40px;\
         background:url(/a.svg) 0 100% no-repeat}}</style>{boxes}"
    );
    let (elapsed, pixels) = shot_time(page, Some(wavy(CUBICS)));
    assert!(elapsed < LIMIT, "screenshot took {elapsed:?}");
    assert!(shows_fill(&pixels), "the drawing did not paint");
}
