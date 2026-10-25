/** Sizing content keywords (css-sizing-3): `min-content`, `max-content`, `fit-content` and
 * `fit-content(<length-percentage>)` on width, min-width and max-width, after Blink's
 * length_utils.cc ResolveInlineLengthInternal. */
use mars_eye::{Document, compute_styles, parse, render};

/** The border-box width of the element with `id`, laid out at 800px. */
fn width(markup: &str, id: &str) -> f64 {
    let document: Document = compute_styles(&parse(markup).unwrap(), &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    let local = document
        .nodes
        .iter()
        .find(|n| n.html_id == id)
        .unwrap()
        .local_id;
    display
        .items
        .iter()
        .find(|i| i.local_id == local)
        .unwrap()
        .width
}

/** Two 60px inline-blocks with no space between them: min-content 60, max-content 120. */
const PAIR: &str = "<div style='display:inline-block;width:60px'></div><div style='display:inline-block;width:60px'></div>";

/** The width of `#o` styled with `style` around `PAIR`, inside a 200px block. */
fn sized(style: &str) -> f64 {
    width(
        &format!(
            "<div style='width:200px;font-size:0'><div id=o style='{style}'>{PAIR}</div></div>"
        ),
        "o",
    )
}

/** `width: min-content` is the min-content width, not a shrink-to-fit width. */
#[test]
fn min_content_width_is_the_widest_unbreakable_piece() {
    assert_eq!(sized("width:min-content"), 60.0);
}

/** `width: max-content` is the max-content width even past the containing block. */
#[test]
fn max_content_width_ignores_the_room() {
    assert_eq!(sized("width:max-content;font-size:0"), 120.0);
    assert_eq!(
        width(
            &format!(
                "<div style='width:100px;font-size:0'><div id=o style='width:max-content'>{PAIR}</div></div>"
            ),
            "o"
        ),
        120.0
    );
}

/** `fit-content(<length>)` clamps the limit between min- and max-content. */
#[test]
fn fit_content_limit_is_clamped_by_the_content() {
    assert_eq!(sized("width:fit-content(100px)"), 100.0);
    assert_eq!(sized("width:fit-content(10px)"), 60.0);
    assert_eq!(sized("width:fit-content(500px)"), 120.0);
}

/** A `fit-content(<percentage>)` limit resolves against the containing block. */
#[test]
fn fit_content_percentage_resolves_against_the_container() {
    assert_eq!(sized("width:fit-content(50%)"), 100.0);
}

/** min-width and max-width take content keywords too. */
#[test]
fn min_and_max_width_take_content_keywords() {
    assert_eq!(sized("width:10px;min-width:fit-content(100px)"), 100.0);
    assert_eq!(sized("width:10px;min-width:max-content"), 120.0);
    assert_eq!(sized("width:190px;max-width:fit-content(100px)"), 100.0);
    assert_eq!(sized("width:190px;max-width:min-content"), 60.0);
}

/** A keyword-sized child contributes its keyword size; a cyclic percentage limit behaves as the
 * property's initial value. */
#[test]
fn keyword_sizes_feed_the_parents_intrinsic_size() {
    let outer = |inner: &str| {
        width(
            &format!(
                "<div id=o style='width:min-content;font-size:0'><div style='{inner}'>{PAIR}</div></div>"
            ),
            "o",
        )
    };
    assert_eq!(outer("width:fit-content(100px)"), 100.0);
    assert_eq!(outer("width:fit-content(50%)"), 60.0);
    assert_eq!(outer("width:50px;min-width:fit-content(50%)"), 60.0);
    assert_eq!(outer("width:max-content"), 120.0);
}

/** An auto-width box with `aspect-ratio` and a definite height contributes the width the ratio
 * transfers from that height. */
#[test]
fn aspect_ratio_transfers_a_definite_height_into_the_contribution() {
    let outer = |inner: &str| {
        width(
            &format!("<div id=o style='width:min-content'><div style='{inner}'></div></div>"),
            "o",
        )
    };
    assert_eq!(outer("height:100px;aspect-ratio:1/1"), 100.0);
    assert_eq!(outer("height:50px;aspect-ratio:2/1;max-height:40px"), 80.0);
    assert_eq!(
        outer("height:100px;padding:10px;box-sizing:border-box;aspect-ratio:1/1"),
        100.0
    );
}

/** In a vertical writing mode `width` is the block size, where content keywords behave as the
 * initial value: the box keeps the width it would have without the keyword. */
#[test]
fn block_axis_keywords_behave_as_auto() {
    let plain = sized("writing-mode:vertical-lr;width:fit-content");
    assert_eq!(sized("writing-mode:vertical-lr;width:min-content"), plain);
    assert_eq!(
        sized("writing-mode:vertical-lr;width:120px;max-width:min-content"),
        120.0
    );
}
