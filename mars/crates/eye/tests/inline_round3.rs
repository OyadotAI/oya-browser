/** Round-three rules from the inline builder: no break between two elements' letters, the
 * `::first-line` fonts in line layout (and not on a clamp's ellipsis), and inline content
 * painting after later sibling blocks' backgrounds. */
use mars_eye::{DisplayItem, compute_styles, parse, render};

/** The display items of `markup` styled by `sheet`. */
fn items(markup: &str, sheet: &str) -> Vec<DisplayItem> {
    let document = compute_styles(&parse(markup).unwrap(), &[sheet.to_string()]).unwrap();
    render(&document, 800.0).unwrap().items
}

/** The first item whose runs include `text`. */
fn holding<'a>(items: &'a [DisplayItem], text: &str) -> &'a DisplayItem {
    items
        .iter()
        .find(|item| item.lines.iter().any(|line| line.text == text))
        .unwrap()
}

/** `X</span>X` is one word: with no room for it, it overflows on one line instead of breaking
 * between the elements; a space between them is still a break opportunity. */
#[test]
fn adjacent_elements_letters_do_not_break() {
    let sheet = "div { width: 5px; font-size: 20px }";
    let joined = items("<div><span>X</span>Y</div>", sheet);
    assert_eq!(
        holding(&joined, "X").lines[0].y,
        holding(&joined, "Y").lines[0].y
    );
    let spaced = items("<div><span>X</span> Y</div>", sheet);
    assert!(holding(&spaced, "Y").lines[0].y > holding(&spaced, "X").lines[0].y);
}

/** The first line lays out in the `::first-line` font size and the next line in the block's. */
#[test]
fn first_line_fonts_take_part_in_layout() {
    let sheet = "div { font-size: 10px } div::first-line { font-size: 20px }";
    let laid = items("<div>Wide<br>Wide</div>", sheet);
    let first = holding(&laid, "Wide");
    let second = laid
        .iter()
        .flat_map(|item| item.lines.iter())
        .filter(|line| line.text == "Wide")
        .nth(1)
        .unwrap();
    let firsts = first.lines.iter().find(|line| line.text == "Wide").unwrap();
    assert!((firsts.width - 2.0 * second.width).abs() < 0.5);
    assert!(laid.iter().any(|item| item.font_size == 20.0));
}

/** A clamped first line keeps its ellipsis in the block's own color and size. */
#[test]
fn clamp_ellipsis_keeps_the_blocks_style() {
    let sheet =
        ".c { line-clamp: 1; color: teal } .c::first-line { color: purple; font-size: 2em }";
    let laid = items("<div class=c>Line 1<br>Line 2</div>", sheet);
    let marker = holding(&laid, "\u{2026}");
    assert_eq!(marker.color.as_deref(), Some("teal"));
    assert_eq!(marker.font_size, 16.0);
}

/** An undecorated block's text paints after a later sibling block's background. */
#[test]
fn text_paints_after_later_block_backgrounds() {
    let sheet = "p { margin: 0; height: 5px } .b { background: green }";
    let laid = items("<p>text</p><p class=b></p>", sheet);
    let text = laid.iter().position(|item| !item.lines.is_empty()).unwrap();
    let background = laid
        .iter()
        .position(|item| item.background.as_deref() == Some("green"))
        .unwrap();
    assert!(text > background);
}
