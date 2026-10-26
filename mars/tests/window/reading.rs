/** Real native captures verify synthetic CSS presentation and unchanged long-source reading. */
use super::*;

/// Geometry tolerates subpixel arithmetic without concealing displaced native content.
const TOLERANCE: f64 = 0.01;
/// Native resize callbacks have a bounded opportunity to publish the matching runtime context.
const RESIZE_ATTEMPTS: usize = 40;
/// The short polling interval leaves real AppKit events and the document worker time to run.
const RESIZE_WAIT: Duration = Duration::from_millis(25);
/// The reproduced declaration caps the content at 26 times a 16-point font.
const CONTENT_WIDTH: f64 = 416.0;
/// Authored horizontal padding is two font ems on each side.
const BODY_PADDING: f64 = 32.0;
/// The authored line advance must match both layout and native paint.
const LINE_HEIGHT: f64 = 25.6;
/// The first paragraph keeps its default 1em top margin below the body's padding, as in Chromium,
/// where padding stops a child's margin from collapsing through.
const PARAGRAPH_MARGIN: f64 = 16.0;
/// The retained httpbin paragraph has this complete decoded UTF-8 byte count.
const PARAGRAPH_BYTES: usize = 3588;
/// Sampling away from the page edge excludes scrollbars and the native Mars shell.
const CANVAS_SAMPLE_INSET: f64 = 8.0;
/// A small color allowance tolerates native color conversion while distinguishing both schemes.
const COLOR_TOLERANCE: u8 = 2;

/// Presentation changes preserve editable state and identity while painting the current CSS scheme.
#[test]
#[ignore = "opens the actual native macOS window and captures static reading presentation"]
fn native_static_reading_reflows_and_paints_complete_source() {
    let mut window = Window::start();
    let capabilities = window.call("runtime.capabilities", json!({}));
    window.pids = capabilities["engines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|engine| engine["pid"].as_u64().unwrap() as u32)
        .collect();
    let base = capabilities["demo_url"]
        .as_str()
        .unwrap()
        .trim_end_matches('/');
    let page = window.call(
        "page.navigate",
        json!({"url":format!("{base}/reading/styled"),"control_epoch":1}),
    );
    let mut params = target(&page, "name");
    params["text"] = json!("Retained reading value");
    window.call("page.type", params);
    resize(&mut window, 1100, 760);
    appearance(&mut window, "light");
    let before = window.call("page.inspect", json!({}));
    appearance(&mut window, "dark");
    let dark = assert_styled(&mut window, &before, "dark");
    assert!(
        dark["layout_generation"].as_u64().unwrap() > before["layout_generation"].as_u64().unwrap()
    );
    let stale = window.request("page.screenshot", json!({"tab_id":before["tab_id"],"page_id":before["page_id"],"expected_revision":before["revision"],"layout_generation":before["layout_generation"]}));
    assert_eq!(stale["error"]["code"], "invalid_request");
    capture_canvas(&mut window, 0x22);
    appearance(&mut window, "light");
    assert_styled(&mut window, &before, "light");
    capture_canvas(&mut window, 0xee);
    resize(&mut window, 640, 480);
    let narrow = assert_styled(&mut window, &before, "light");
    assert!(
        narrow["layout_generation"].as_u64().unwrap() > dark["layout_generation"].as_u64().unwrap()
    );
    capture_canvas(&mut window, 0xee);
    assert_long_reading(&mut window, &format!("{base}/reading/long"));
}

/// An explicit theme command returns only after its normal native presentation acknowledgment.
fn appearance(window: &mut Window, value: &str) {
    window.call(
        "window.appearance",
        json!({"appearance":value,"control_epoch":1}),
    );
}

/// Actual viewport and runtime context must converge before captures or CSS geometry assertions.
fn resize(window: &mut Window, width: usize, height: usize) {
    window.call(
        "window.resize",
        json!({"width":width,"height":height,"control_epoch":1}),
    );
    for _ in 0..RESIZE_ATTEMPTS {
        let state = window.call("window.inspect", json!({}));
        let viewport = &state["page"]["viewport"];
        let context = &state["render_context"];
        let native = &state["native_banner"]["page_content"];
        if number(&state, "width") == width as f64
            && number(&state, "height") == height as f64
            && viewport["width"] == context["width"]
            && viewport["height"] == context["height"]
            && (number(context, "height") - number(native, "height")).abs() <= TOLERANCE
            && (number(context, "width") - number(native, "width")).abs() <= TOLERANCE
        {
            return;
        }
        thread::sleep(RESIZE_WAIT);
    }
    panic!("native reading viewport did not converge to {width}x{height}");
}

/// The shared display geometry uses the actual viewport and keeps input/page identities unchanged.
fn assert_styled(window: &mut Window, before: &Value, appearance: &str) -> Value {
    let page = window.call("page.inspect", json!({}));
    let state = window.call("window.inspect", json!({}));
    let context = &state["render_context"];
    assert_eq!(context["appearance"], appearance);
    assert_eq!(context["width"], page["viewport"]["width"]);
    assert_eq!(context["height"], page["viewport"]["height"]);
    assert_eq!(
        state["page"]["layout_generation"],
        page["layout_generation"]
    );
    assert_eq!(page["page_id"], before["page_id"]);
    assert_eq!(page["revision"], before["revision"]);
    assert_eq!(
        node(&page, "name")["node_id"],
        node(before, "name")["node_id"]
    );
    assert_eq!(node(&page, "name")["value"], "Retained reading value");
    assert_eq!(
        node(&page, "first")["node_id"],
        node(before, "first")["node_id"]
    );
    let layout = window.call("page.layout", json!({}));
    let first = layout["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["node_id"] == node(&page, "first")["node_id"])
        .unwrap();
    close(
        number(first, "x"),
        (number(context, "width") - CONTENT_WIDTH - BODY_PADDING * 2.0) / 2.0 + BODY_PADDING,
    );
    close(number(first, "width"), CONTENT_WIDTH);
    close(
        number(first, "y"),
        number(context, "height") / 4.0 + PARAGRAPH_MARGIN,
    );
    close(number(first, "line_height"), LINE_HEIGHT);
    assert_nested_link_frame(&state, &layout, &page);
    page
}

/// A centered link containing only span text keeps its native activation frame on the painted union.
fn assert_nested_link_frame(state: &Value, layout: &Value, page: &Value) {
    let items = layout["items"].as_array().unwrap();
    let link = items
        .iter()
        .find(|item| item["node_id"] == node(page, "back")["node_id"])
        .unwrap();
    assert!(link["lines"].as_array().unwrap().is_empty());
    let span = items.iter().find(|item| item["tag"] == "span").unwrap();
    let lines = span["lines"].as_array().unwrap();
    assert!(!lines.is_empty());
    let left = lines
        .iter()
        .map(|line| number(line, "x"))
        .fold(f64::INFINITY, f64::min);
    let right = lines
        .iter()
        .map(|line| number(line, "x") + number(line, "width"))
        .fold(f64::NEG_INFINITY, f64::max);
    let native = state["native_frames"]
        .as_array()
        .unwrap()
        .iter()
        .find(|frame| frame["html_id"] == "back")
        .unwrap();
    close(number(link, "x"), left);
    close(number(link, "width"), right - left);
    close(number(native, "x"), left);
    close(number(native, "width"), right - left);
}

/// The unmodified httpbin source remains one complete paragraph through top/middle/end scrolling.
fn assert_long_reading(window: &mut Window, url: &str) {
    let page = window.call("page.navigate", json!({"url":url,"control_epoch":1}));
    let paragraphs = page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|node| node["tag"] == "p")
        .collect::<Vec<_>>();
    assert_eq!(paragraphs.len(), 1);
    let paragraph = paragraphs[0];
    let text = paragraph["text"].as_str().unwrap();
    assert_eq!(text.len(), PARAGRAPH_BYTES);
    assert!(
        text.trim()
            .starts_with("Availing himself of the mild, summer-cool weather")
    );
    assert!(text.contains("It was the Bottle Conjuror!"));
    assert!(
        text.trim()
            .ends_with("and all of them a care-killing competency.")
    );
    let layout = window.call("page.layout", json!({}));
    let lines = layout["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| item["node_id"] == paragraph["node_id"])
        .flat_map(|item| item["lines"].as_array().unwrap())
        .collect::<Vec<_>>();
    assert_source_coverage(text, &lines);
    let extent = number(&layout, "height");
    let maximum = (extent - number(&page["viewport"], "height")).max(0.0);
    assert!(maximum > 0.0);
    for y in [0.0, maximum / 2.0, extent] {
        let current = window.call("page.scroll", json!({"page_id":page["page_id"],"expected_revision":page["revision"],"y":y,"control_epoch":1}));
        assert_eq!(current["page_id"], page["page_id"]);
        assert_eq!(current["revision"], page["revision"]);
        assert_eq!(current["layout_generation"], page["layout_generation"]);
        assert!(
            current["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|node| node["node_id"] == paragraph["node_id"] && node["text"] == text)
        );
        close(number(&current["viewport"], "scroll_y"), y.min(maximum));
        window.capture();
    }
}

/// Every painted line consumes the normalized source in order; a wrap may omit its boundary
/// space, but a break after a source hyphen must not invent one or lose any source characters.
pub(super) fn assert_source_coverage(source: &str, lines: &[&Value]) {
    let normalized = source.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut remaining = normalized.as_str();
    assert!(!lines.is_empty(), "the source has no painted lines");
    for (index, line) in lines.iter().enumerate() {
        let text = line["text"].as_str().unwrap();
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(!text.is_empty(), "painted line {index} is empty");
        remaining = remaining.strip_prefix(' ').unwrap_or(remaining);
        remaining = remaining.strip_prefix(&text).unwrap_or_else(|| {
            panic!("painted line {index} {text:?} does not consume source {remaining:?}")
        });
    }
    assert_eq!(remaining, "", "painted lines omit the end of the source");
}

/// A blank point in the actual native page viewport must carry the authored root canvas color.
fn capture_canvas(window: &mut Window, channel: u8) {
    let capture = window.capture();
    let state = window.call("window.inspect", json!({}));
    let page = &state["native_banner"]["page"];
    let x = number(page, "x") + CANVAS_SAMPLE_INSET;
    let y =
        number(&state, "height") - number(page, "y") - number(page, "height") + CANVAS_SAMPLE_INSET;
    let file = std::fs::File::open(capture["path"].as_str().unwrap()).unwrap();
    let mut decoder = png::Decoder::new(BufReader::new(file));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().unwrap();
    let mut bytes = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut bytes).unwrap();
    let px = (x * info.width as f64 / number(&state, "width")).floor() as usize;
    let py = (y * info.height as f64 / number(&state, "height")).floor() as usize;
    let channels = info.color_type.samples();
    assert!(channels >= 3);
    let offset = (py * info.width as usize + px) * channels;
    assert!(
        bytes[offset..offset + 3]
            .iter()
            .all(|value| value.abs_diff(channel) <= COLOR_TOLERANCE),
        "canvas pixel {:?}, expected {channel}; capture {}",
        &bytes[offset..offset + 3],
        capture["path"]
    );
}

/// Semantic fixture IDs survive every layout generation and avoid parser-local assumptions.
fn node<'a>(page: &'a Value, id: &str) -> &'a Value {
    page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["html_id"] == id)
        .unwrap()
}

/// Geometry lookup failures retain the actual native observation in their assertion message.
fn number(value: &Value, key: &str) -> f64 {
    value[key]
        .as_f64()
        .unwrap_or_else(|| panic!("missing native {key} in {value}"))
}

/// Shared point comparisons describe the actual displaced coordinate when a check fails.
fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= TOLERANCE,
        "actual {actual}, expected {expected}"
    );
}
