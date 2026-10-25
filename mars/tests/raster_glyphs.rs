/** The windowless painter draws real glyphs, pixel-snaps boxes as Blink does, and reports a web
font that failed to load instead of silently falling back. */
#[allow(dead_code)]
mod support;
use serde_json::{Value, json};
use support::{Fixture, agent_call, desktop, html};

/** A screenshot of a fixture page whose body is `body`: (width, RGB rows, navigate result). */
fn shoot(body: &str) -> (usize, Vec<u8>, Value) {
    let page: &'static str = Box::leak(body.to_string().into_boxed_str());
    let fixture = Fixture::routes(vec![("/", html("", page))]);
    let mut runtime = desktop(&fixture.url);
    agent_call(
        &mut runtime,
        "page.emulate",
        json!({"viewport": {"width": 400.0, "height": 300.0}}),
    )
    .unwrap();
    let navigated = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    let shot = agent_call(&mut runtime, "page.screenshot", json!({})).unwrap();
    let png = decode_base64(shot["png_base64"].as_str().unwrap());
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png));
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().unwrap();
    let mut pixels = vec![0; reader.output_buffer_size()];
    let frame = reader.next_frame(&mut pixels).unwrap();
    pixels.truncate(frame.buffer_size());
    (frame.width as usize, pixels, navigated)
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

/** Bars painted every run of the same width alike; glyphs differ by letter. */
#[cfg(target_os = "macos")]
#[test]
fn text_paints_as_glyphs_not_bars() {
    let style = "<style>body{margin:0;font:20px/1 monospace}</style>";
    let (_, xs, _) = shoot(&format!("{style}XXXX"));
    let (_, os, _) = shoot(&format!("{style}OOOO"));
    assert_ne!(xs, os);
    // Antialiased edges leave partial coverage, which a solid bar never has.
    assert!(xs.chunks(3).any(|p| p[0] > 0 && p[0] < 255));
}

/** A box at a half-pixel offset covers as many rows as it is tall, snapped as Blink snaps it. */
#[test]
fn half_pixel_boxes_keep_their_height() {
    let body = "<style>body{margin:0}</style><div style='height:10.5px'></div>\
        <div style='height:20px;width:10px;background:black'></div>";
    let (width, pixels, _) = shoot(body);
    let black = (0..100)
        .filter(|row| pixels[row * width * 3] == 0)
        .collect::<Vec<_>>();
    assert_eq!(black.len(), 20, "rows {black:?}");
    assert_eq!(black[0], 11);
}

/** A web font that cannot be fetched is reported, not silently replaced. */
#[test]
fn a_failed_web_font_is_reported() {
    let body = "<style>@font-face{font-family:Gone;src:url(/gone.ttf)}\
        body{font-family:Gone}</style>text";
    let (_, _, navigated) = shoot(body);
    let text = navigated.to_string();
    assert!(
        text.contains("A web font failed to load"),
        "no font diagnostic in {text}"
    );
}
