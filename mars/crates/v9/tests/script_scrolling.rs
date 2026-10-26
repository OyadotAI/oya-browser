/** Script scrolling: `scrollTop`/`scrollLeft` and `scroll*()` on elements and the window are
recorded for the app to paint, clamped to the scroll range the last layout reported. */
mod common;

/** An element's `scrollTop` and `scrollLeft` round-trip and reach the app as one offset. */
#[test]
fn element_scroll_offsets_are_recorded_for_the_app() {
    let execution = common::load(
        "const main = document.getElementById('main');
         main.scrollTop = 50; main.scrollLeft = 20; main.scrollBy(0, 5);
         const r = [main.scrollTop, main.scrollLeft].join('|');",
    );
    assert_eq!(common::failure(&execution), None);
    assert_eq!(common::global(&execution, "r"), "55|20");
    assert_eq!(
        execution.effects.element_scroll.get(&0),
        Some(&[20.0, 55.0])
    );
}

/** A laid-out container scrolls no further than its range, and one that did not overflow
cannot scroll, so a loop that scrolls until `scrollTop` stops growing ends. */
#[test]
fn script_scrolls_clamp_to_the_last_layouts_range() {
    let mut environment = common::environment();
    environment.laid_out_before = Some(4);
    environment.scroll_max.insert(0, [0.0, 80.0]);
    let execution = common::load_with(
        "const main = document.getElementById('main'), first = document.getElementById('first');
         main.scrollTo({top: 1000}); first.scrollTop = 30;
         const r = [main.scrollTop, first.scrollTop].join('|');",
        &environment,
    );
    assert_eq!(common::failure(&execution), None);
    assert_eq!(common::global(&execution, "r"), "80|0");
}

/** `window.scrollTo` moves the document scroll offset that `scrollY` reads and the app adopts. */
#[test]
fn window_scroll_to_moves_the_viewport() {
    let execution = common::load("scrollTo(0, 300); const r = String(scrollY);");
    assert_eq!(common::failure(&execution), None);
    assert_eq!(common::global(&execution, "r"), "300");
    assert_eq!(execution.effects.scroll_y, Some(300.0));
}

/** The viewport scrolls horizontally too: `scrollLeft` on the scrolling element clamps to the
range the app gave (negative in a vertical-rl page), and `scrollWidth` reports the document's
scrollable width. */
#[test]
fn the_viewport_scrolls_horizontally_within_its_range() {
    let mut environment = common::environment();
    environment.scroll_x_range = Some([-300.0, 0.0]);
    let execution = common::load_with(
        "const root = document.scrollingElement;
         const width = root.scrollWidth;
         root.scrollLeft = -1000;
         const r = [width, root.scrollLeft, scrollX].join('|');",
        &environment,
    );
    assert_eq!(common::failure(&execution), None);
    let width = environment.viewport_width + 300.0;
    assert_eq!(
        common::global(&execution, "r"),
        format!("{width}|-300|-300")
    );
    assert_eq!(execution.effects.scroll_x, Some(-300.0));
}
