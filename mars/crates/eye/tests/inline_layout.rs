/** Inline layout after Blink's InlineLayoutAlgorithm: the first-line `text-indent`, per-space
`word-spacing`, and text-align over the line's start edge. */
use mars_eye::{PaintLine, compute_styles, parse, render};

/** The painted runs of `#t` after laying out `markup` at 800px. */
fn runs(markup: &str) -> Vec<PaintLine> {
    let markup = format!("<body style='margin:0'>{markup}");
    let document = compute_styles(&parse(&markup).unwrap(), &[]).unwrap();
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
        .filter(|i| i.local_id == local)
        .flat_map(|i| i.lines.clone())
        .collect()
}

/** text-indent moves only the first line of its block, by a length. */
#[test]
fn text_indent_moves_the_first_line_only() {
    let found =
        runs("<div id=t style='width:100px;text-indent:30px'>aaa bbb ccc ddd eee fff ggg</div>");
    assert!(found.len() > 1, "{found:?}");
    assert_eq!(found[0].x, 30.0);
    assert_eq!(found[1].x, 0.0);
}

/** A percentage indent resolves against the block's content width. */
#[test]
fn text_indent_percentage_is_of_the_container_width() {
    let found = runs("<div id=t style='width:200px;text-indent:10%'>word</div>");
    assert_eq!(found[0].x, 20.0);
}

/** An inherited indent reaches a span's text on its block's first line, and a line after a block
child is not the first formatted line. */
#[test]
fn text_indent_skips_lines_after_a_block_child() {
    let first = runs("<div style='text-indent:40px'><span id=t>word</span></div>");
    assert_eq!(first[0].x, 40.0);
    let after =
        runs("<div style='text-indent:40px'><p style='margin:0'>a</p><span id=t>word</span></div>");
    assert_eq!(after[0].x, 0.0);
}

/** word-spacing widens every space between words. */
#[test]
fn word_spacing_widens_each_space() {
    let plain = runs("<div id=t>a b c</div>")[0].width;
    let spaced = runs("<div id=t style='word-spacing:10px'>a b c</div>")[0].width;
    assert!((spaced - plain - 20.0).abs() < 0.01, "{plain} {spaced}");
}

/** Centered text with an indent centers what is left after the indent. */
#[test]
fn centered_line_keeps_its_indent_inside_the_line() {
    let plain = runs("<div id=t style='width:200px;text-align:center'>word</div>")[0].x;
    let indented =
        runs("<div id=t style='width:200px;text-align:center;text-indent:40px'>word</div>")[0].x;
    assert!((indented - plain - 20.0).abs() < 0.01, "{plain} {indented}");
}

/** A no-break space does not collapse with its neighbours: ten of them stay ten spaces wide. */
#[test]
fn no_break_spaces_do_not_collapse() {
    let one = runs("<div id=t>a b</div>")[0].width;
    let ten =
        runs("<div id=t>a &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;b</div>")[0].width;
    assert!(ten > one * 2.0, "{one} {ten}");
}

/** An ideographic space does not collapse but is a break opportunity (UAX #14 BA). */
#[test]
fn an_ideographic_space_is_a_break_opportunity() {
    let found = runs("<div id=t style='width:60px'>aaa\u{3000}bbbbbbbbbbbb</div>");
    assert_eq!(found[0].text, "aaa\u{3000}", "{found:?}");
    assert_eq!(found.len(), 2, "{found:?}");
}
