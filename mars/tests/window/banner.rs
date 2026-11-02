/** Public native form validation verifies the complete banner in actual wide and narrow windows. */
use super::*;

/// Native point comparisons permit platform rounding without concealing a clipped text line.
const TOLERANCE: f64 = 0.01;
/// The toolbar and tab strip together define the first available error-banner coordinate.
const OCCUPIED_MARS: f64 = 86.0;
/// Complete messages retain consistent spacing around text and native recovery controls.
const PADDING: f64 = 8.0;
/// A visible error must preserve a usable page viewport.
const MINIMUM_PAGE: f64 = 120.0;
/// Resize returns before the worker has committed its new presentation context.
const PRESENTATION_ATTEMPTS: usize = 40;
/// A short bounded interval lets the real native UI and owned runtime publish together.
const PRESENTATION_WAIT: Duration = Duration::from_millis(25);

/// Invalid submission keeps its native recovery message readable without creating an agent bypass.
#[test]
#[ignore = "opens the actual native macOS window and captures complete recovery banners"]
fn native_validation_banner_is_complete_at_wide_and_narrow_sizes() {
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
        json!({"url":format!("{base}/forms"),"control_epoch":1}),
    );
    let failed = window.request("page.click", target(&page, "submit"));
    assert_eq!(failed["error"]["code"], "validation_error");
    for (width, height) in [(1100, 760), (640, 480)] {
        window.call(
            "window.resize",
            json!({"width":width,"height":height,"control_epoch":1}),
        );
        let state = wait_presentation(&mut window, width as f64, height as f64, true);
        assert_eq!(state["page"]["page_id"], page["page_id"]);
        assert_complete(&state, width as f64, height as f64);
        window.capture();
    }
    window.call(
        "page.navigate",
        json!({"url":format!("{base}/"),"control_epoch":1}),
    );
    let state = wait_presentation(&mut window, 640.0, 480.0, false);
    assert!(state["error"].is_null(), "{state}");
    assert_eq!(state["native_banner"]["visible"], false);
    let page = &state["native_banner"]["page"];
    close(
        number(page, "y") + number(page, "height"),
        number(&state, "height") - OCCUPIED_MARS,
    );
    window.capture();
}

/// Banner appearance/removal and resizing both change the page viewport asynchronously.
fn wait_presentation(window: &mut Window, width: f64, height: f64, visible: bool) -> Value {
    for _ in 0..PRESENTATION_ATTEMPTS {
        let state = window.call("window.inspect", json!({}));
        let native = &state["native_banner"]["page_content"];
        let context = &state["render_context"];
        let viewport = &state["page"]["viewport"];
        if (number(&state, "width") - width).abs() <= TOLERANCE
            && (number(&state, "height") - height).abs() <= TOLERANCE
            && state["native_banner"]["visible"] == visible
            && viewport["width"] == context["width"]
            && viewport["height"] == context["height"]
            && (number(context, "width") - number(native, "width")).abs() <= TOLERANCE
            && (number(context, "height") - number(native, "height")).abs() <= TOLERANCE
        {
            return state;
        }
        thread::sleep(PRESENTATION_WAIT);
    }
    panic!("native banner presentation did not converge to {width}x{height}");
}

/// Native view coordinates confirm every line, fixed action and document region is disjoint.
fn assert_complete(state: &Value, width: f64, height: f64) {
    close(number(state, "width"), width);
    close(number(state, "height"), height);
    let banner = &state["native_banner"];
    assert_eq!(banner["visible"], true, "{state}");
    assert_eq!(banner["message"], state["error"]);
    assert!(
        banner["message"]
            .as_str()
            .unwrap()
            .contains("Focus the required field and provide its value.")
    );
    assert_eq!(banner["message"].as_str().unwrap().lines().count(), 3);
    assert_eq!(banner["retry_visible"], false);
    assert_eq!(banner["scrolling"], false, "{banner}");
    let text = &banner["text_viewport"];
    let dismiss = &banner["dismiss"];
    let page = &banner["page"];
    close(
        number(text, "y") + number(text, "height"),
        height - OCCUPIED_MARS - PADDING,
    );
    close(
        number(text, "y"),
        number(dismiss, "y") + number(dismiss, "height") + PADDING,
    );
    close(
        number(dismiss, "y"),
        number(page, "y") + number(page, "height") + PADDING,
    );
    close(
        width - number(dismiss, "x") - number(dismiss, "width"),
        16.0,
    );
    assert!(number(page, "height") >= MINIMUM_PAGE);
    assert!(
        number(&banner["text_document"], "height")
            <= number(&banner["text_clip"], "height") + TOLERANCE,
        "{banner}"
    );
    close(number(&banner["text_clip"], "y"), 0.0);
}

/// Frame lookup failures identify the actual observation that omitted native geometry.
fn number(value: &Value, key: &str) -> f64 {
    value[key]
        .as_f64()
        .unwrap_or_else(|| panic!("native {key} is missing from {value}"))
}

/// A common point tolerance produces useful actual-versus-expected assertion output.
fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= TOLERANCE,
        "actual {actual}, expected {expected}"
    );
}
