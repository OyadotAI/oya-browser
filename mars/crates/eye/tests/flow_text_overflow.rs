/** `text-overflow: ellipsis` after Blink's LineTruncator and css-overflow-3: only clipping block
containers ellipse, the ellipsis ends inside the line box, and later runs are hidden. */
use mars_eye::{DisplayList, Document, compute_styles, parse, render};

/** Laid-out document and display list for `markup` at 800px with no body margin. */
fn layout(markup: &str) -> (Document, DisplayList) {
    let markup = format!("<body style='margin:0;font-size:16px'>{markup}");
    let document = compute_styles(&parse(&markup).unwrap(), &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    (document, display)
}

/** Every painted run as (text, x, width), in item order. */
fn runs(markup: &str) -> Vec<(String, f64, f64)> {
    let (_, display) = layout(markup);
    let runs = display.items.iter().flat_map(|item| item.lines.iter());
    runs.filter(|line| !line.text.is_empty())
        .map(|line| (line.text.clone(), line.x, line.width))
        .collect()
}

const CLIPPED: &str = "width:100px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis";

/** An overflowing line ends in "…" and the kept text plus the ellipsis fit the line box. */
#[test]
fn an_overflowing_line_ends_in_an_ellipsis_inside_the_box() {
    let found = runs(&format!(
        "<div style='{CLIPPED}'>How to read this documentation</div>"
    ));
    let (text, x, width) = &found[0];
    assert!(
        text.ends_with('\u{2026}') && text.starts_with("How"),
        "{found:?}"
    );
    assert!(x + width <= 100.01, "{found:?}");
    assert!(text.chars().count() > 3, "kept as much as fits: {found:?}");
}

/** A line that fits is left alone. */
#[test]
fn a_fitting_line_is_not_truncated() {
    assert_eq!(
        runs(&format!("<div style='{CLIPPED}'>Short</div>"))[0].0,
        "Short"
    );
}

/** Without a clipping overflow, `text-overflow` does nothing. */
#[test]
fn visible_overflow_does_not_ellipse() {
    let style = "width:100px;white-space:nowrap;text-overflow:ellipsis";
    let found = runs(&format!(
        "<div style='{style}'>How to read this documentation</div>"
    ));
    assert_eq!(found[0].0, "How to read this documentation");
}

/** Runs of inline children after the cut are hidden, and the ellipsis follows the kept content. */
#[test]
fn runs_after_the_ellipsis_are_hidden() {
    let markup = format!("<div style='{CLIPPED}'>Hi <b>world</b> and a lot more text</div>");
    let found = runs(&markup);
    assert!(found.iter().all(|(_, x, _)| *x < 100.0), "{found:?}");
    assert_eq!(
        found
            .iter()
            .filter(|(text, ..)| text.contains('\u{2026}'))
            .count(),
        1
    );
}

/** The first character of a line is clipped, never replaced by the ellipsis. */
#[test]
fn the_first_character_stays() {
    let style = "width:4px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis";
    let found = runs(&format!("<div style='{style}'>Wide</div>"));
    assert!(found[0].0.starts_with('W'), "{found:?}");
}
