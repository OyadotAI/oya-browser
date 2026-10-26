/** `::first-letter`, `initial-letter` and `::first-line` rules from the inline builder's pass. */
use mars_eye::{DisplayItem, compute_styles, parse, render};

/** The display items of `markup` styled by `sheet`. */
fn items(markup: &str, sheet: &str) -> Vec<DisplayItem> {
    let document = compute_styles(&parse(markup).unwrap(), &[sheet.to_string()]).unwrap();
    render(&document, 800.0).unwrap().items
}

/** The first-letter boxes' texts. */
fn letters(items: &[DisplayItem]) -> Vec<String> {
    items
        .iter()
        .filter(|item| item.tag == "::first-letter")
        .flat_map(|item| item.lines.iter().map(|line| line.text.clone()))
        .collect()
}

/** The first letter, with the punctuation around it, takes the `::first-letter` style and the
 * rest of the text follows it on the line. */
#[test]
fn first_letter_splits_the_leading_letter_and_punctuation() {
    let laid = items("<p>\"Hello,\" said</p>", "p::first-letter { color: green }");
    assert_eq!(letters(&laid), vec!["\"H".to_string()]);
    let letter = laid
        .iter()
        .find(|item| item.tag == "::first-letter")
        .unwrap();
    assert_eq!(letter.color.as_deref(), Some("green"));
    let paragraph = laid.iter().find(|item| item.tag == "p").unwrap();
    assert_eq!(paragraph.lines[0].text, "ello,\" said");
    assert!(paragraph.lines[0].x >= letter.x + letter.width - 0.01);
}

/** The letter is found in the first text of the first line, past floats and into a first
 * child block; text after other content, or after a non-collapsible space, has none. */
#[test]
fn first_letter_reaches_the_first_text_only() {
    let sheet = "div::first-letter { color: green } span { float: left; width: 5px; height: 5px }";
    assert_eq!(
        letters(&items("<div><span></span>Abc</div>", sheet)),
        vec!["A"]
    );
    assert_eq!(
        letters(&items("<div><p>Abc</p><p>Def</p></div>", sheet)),
        vec!["A"]
    );
    assert!(letters(&items("<div><img width=5 height=5>Abc</div>", sheet)).is_empty());
    assert!(letters(&items("<div>&nbsp;Abc</div>", sheet)).is_empty());
}

/** A drop initial three lines tall: its cap height spans two lines plus a cap, its box floats
 * at the line start, and the first line's text sits beside it. */
#[test]
fn initial_letter_floats_beside_the_lines_it_spans() {
    let sheet = "div { font-size: 20px; line-height: 24px; width: 300px } div::first-letter { initial-letter: 3 }";
    let laid = items("<div>Abc<br>def<br>ghi<br>jkl</div>", sheet);
    let letter = laid
        .iter()
        .find(|item| item.tag == "::first-letter")
        .unwrap();
    assert!(letter.font_size > 3.0 * 20.0, "sized to the lines it spans");
    let block = laid.iter().find(|item| item.tag == "div").unwrap();
    assert!(block.lines[0].x >= letter.x + letter.width - 0.01);
    let third = laid
        .iter()
        .find(|item| item.lines.iter().any(|line| line.text == "ghi"))
        .unwrap();
    assert!(
        third.lines[0].x >= letter.x + letter.width - 0.01,
        "the third line is beside it"
    );
}

/** `initial-letter: 3 raise` lowers the first line by the two lines the letter rises above it. */
#[test]
fn raised_initial_lowers_the_first_line() {
    let sheet = "div { font-size: 20px; line-height: 24px; width: 300px } div::first-letter { initial-letter: 3 raise }";
    let laid = items("<div>Abc</div>", sheet);
    let block = laid.iter().find(|item| item.tag == "div").unwrap();
    assert!((block.lines[0].y - block.y - 48.0).abs() < 1.0);
}

/** With `text-indent`, the box sits indented twice from the line start (as Blink places it),
 * and the first line's text and the next line's both start right after it. */
#[test]
fn indented_initial_letter_sits_twice_indented() {
    let sheet = "div { font-size: 20px; line-height: 24px; width: 300px; text-indent: 10px } div::first-letter { initial-letter: 2 }";
    let laid = items("<div>Abc<br>def</div>", sheet);
    let block = laid.iter().find(|item| item.tag == "div").unwrap();
    let letter = laid
        .iter()
        .find(|item| item.tag == "::first-letter")
        .unwrap();
    assert!((letter.x - block.x - 20.0).abs() < 0.01);
    let after = letter.x + letter.width;
    assert!((block.lines[0].x - after).abs() < 0.01, "first line");
    let second = laid
        .iter()
        .find(|item| item.lines.iter().any(|l| l.text == "def"))
        .unwrap();
    assert!((second.lines[0].x - after).abs() < 0.01, "second line");
}

/** The clearfix: a `visibility: hidden` `::after` still lays out and clears the floats, so
 * its element grows around them, but paints no text. */
#[test]
fn a_hidden_after_box_still_clears_floats() {
    let sheet = "span { float: left; width: 10px; height: 50px } div::after { content: 'x'; display: block; visibility: hidden; clear: both; height: 0 }";
    let laid = items("<div><span></span></div>", sheet);
    let block = laid.iter().find(|item| item.tag == "div").unwrap();
    assert!(block.height >= 50.0, "{}", block.height);
    let after = laid.iter().find(|item| item.tag == "::after").unwrap();
    // Invisible boxes paint at opacity 0 (or, before that, in transparent ink).
    let painted = after.opacity > 0.0 && after.color.as_deref() != Some("transparent");
    assert!(!painted, "{:?} {}", after.color, after.opacity);
}

/** A float met after an initial letter starts below it, never beside it. */
#[test]
fn a_later_float_clears_the_initial_letter() {
    let sheet = "div { font-size: 20px; line-height: 24px; width: 300px } div::first-letter { initial-letter: 3 } span { float: left; width: 10px; height: 10px }";
    let laid = items("<div>Abc<br><span></span>def</div>", sheet);
    let letter = laid
        .iter()
        .find(|item| item.tag == "::first-letter")
        .unwrap();
    let float = laid.iter().find(|item| item.tag == "span").unwrap();
    assert!(float.y >= letter.y + letter.height - 0.01);
}

/** The first formatted line's text takes the `::first-line` color, inherited colors only: an
 * item spanning two lines splits into a recolored copy for its first-line runs; an explicitly
 * colored descendant and a grid container are untouched. */
#[test]
fn first_line_recolors_the_first_line_only() {
    let sheet = "p { width: 60px; color: red } p::first-line { color: green } b { color: blue }";
    let laid = items("<p>aaaa bbbb cccc <b>x</b></p>", sheet);
    let green: Vec<&str> = laid
        .iter()
        .filter(|item| item.color.as_deref() == Some("green"))
        .flat_map(|item| item.lines.iter().map(|line| line.text.as_str()))
        .collect();
    assert_eq!(green, vec!["aaaa"]);
    assert!(
        laid.iter()
            .any(|item| item.color.as_deref() == Some("blue"))
    );
    let grid = items(
        "<div>a</div>",
        "div { display: grid } div::first-line { color: green }",
    );
    assert!(
        grid.iter()
            .all(|item| item.color.as_deref() != Some("green"))
    );
}
