/** Min- and max-content widths after Blink's intrinsic sizing: generated `::before`/`::after`
boxes are children of their element in the layout tree, so they count toward its width. */
use mars_eye::{compute_styles, parse, render};

/** Border-box width of `#t` after laying out `markup` at 800px. */
fn width(css: &str, markup: &str) -> f64 {
    let markup = format!("<body style='margin:0;font-size:16px'>{markup}");
    let document = compute_styles(&parse(&markup).unwrap(), &[css.to_string()]).unwrap();
    let display = render(&document, 800.0).unwrap();
    let local = document
        .nodes
        .iter()
        .find(|n| n.html_id == "t")
        .unwrap()
        .local_id;
    display
        .items
        .iter()
        .find(|i| i.local_id == local)
        .unwrap()
        .width
}

/** A float that holds only generated text shrinks to fit that text plus its padding. */
#[test]
fn generated_content_widens_a_shrink_to_fit_float() {
    let css = "#t{float:right;padding:0 10px}#t::after{content:'More links'}";
    let found = width(css, "<a id=t href=#></a>");
    assert!(found > 60.0, "{found}");
}

/** `::before` and the element's own text share one line in the max-content width. */
#[test]
fn generated_and_own_text_add_up_on_one_line() {
    let plain = width("#t{float:left}", "<span id=t>Label</span>");
    let both = width(
        "#t{float:left}#t::before{content:'Prefix '}",
        "<span id=t>Label</span>",
    );
    assert!(both > plain + 30.0, "{plain} {both}");
}

/** Spaces between inline runs count once per boundary, whichever run holds them, so linked
 * words separated by " | " are as wide as the same plain text (HN's top bar), within the
 * 1/64 px each run rounds up to. */
#[test]
fn spaces_between_runs_count_once_each() {
    let linked = width(
        "#t{float:left}",
        "<div id=t><a href=#>new</a> | <a href=#>past</a></div>",
    );
    let plain = width("#t{float:left}", "<div id=t>new | past</div>");
    assert!((linked - plain).abs() < 0.05, "{linked} {plain}");
    let words = width(
        "#t{float:left}",
        "<div id=t><span>ab</span> c <span>d</span></div>",
    );
    let text = width("#t{float:left}", "<div id=t>ab c d</div>");
    assert!((words - text).abs() < 0.05, "{words} {text}");
}
