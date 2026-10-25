/** Blink's EnsureTextMetrics for forced breaks: a `<br>` directly in a block sits in the root
inline box, which always carries the strut, so the br's own font-size and line-height never
change the line (css-inline br-line-height and br-font-size). */
use mars_eye::{compute_styles, parse, render};

/** The height of the element with `id` after laying out `markup` at 800px. */
fn height(markup: &str, id: &str) -> f64 {
    let markup = format!("<body style='margin:0'>{markup}");
    let document = compute_styles(&parse(&markup).unwrap(), &[]).unwrap();
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
        .height
}

/** A br's own line-height does not stretch its line. */
#[test]
fn br_line_height_is_ignored_in_the_root_box() {
    let m =
        "<style>br{line-height:5}</style><div id=d style='font:25px/1 serif'>XX<br>XX<br>XX</div>";
    assert_eq!(height(m, "d"), 75.0);
}

/** A br's own font-size does not stretch its line. */
#[test]
fn br_font_size_is_ignored_in_the_root_box() {
    let m =
        "<style>br{font-size:50px}</style><div id=d style='font:10px/1 serif'>XX<br>XX<br>XX</div>";
    assert_eq!(height(m, "d"), 30.0);
}

/** An empty line made only by a br still keeps the strut's height. */
#[test]
fn br_only_line_keeps_the_strut() {
    let m = "<style>br{line-height:5}</style><div id=d style='font:20px/1 serif'><br><br>X</div>";
    assert_eq!(height(m, "d"), 60.0);
}
