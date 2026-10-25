/** Table column widths after Blink's table_layout_utils.cc (css-tables-3 width distribution).
Expected numbers are what Chromium lays out for the same markup in 16px sans-serif. */
use mars_eye::{compute_styles, parse, render};

/** (x, width) of `#id` after laying out `markup` at 1100px with no body margin. */
fn place(markup: &str, id: &str) -> (f64, f64) {
    let markup = format!("<body style='margin:0;font:16px sans-serif'>{markup}");
    let document = compute_styles(&parse(&markup).unwrap(), &[]).unwrap();
    let display = render(&document, 1100.0).unwrap();
    let local = document
        .nodes
        .iter()
        .find(|n| n.html_id == id)
        .unwrap()
        .local_id;
    let item = display.items.iter().find(|i| i.local_id == local).unwrap();
    (item.x, item.width)
}

/** Within half a pixel of Chromium. */
fn near(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() < 0.5
}

/** A colspan cell's excess goes to its columns in proportion to their max widths, not evenly. */
#[test]
fn colspan_excess_follows_max_widths() {
    let markup = "<table border=1><tr><td id=a>one</td><td id=b>two words here</td></tr>\
                  <tr><td colspan=2>spanning cell content</td></tr></table>";
    assert!(
        near(place(markup, "a").1, 32.65),
        "{:?}",
        place(markup, "a")
    );
    assert!(
        near(place(markup, "b").1, 119.68),
        "{:?}",
        place(markup, "b")
    );
}

/** A percentage column takes its share of the table; an authored length column keeps its width. */
#[test]
fn percentage_and_length_columns() {
    let markup = "<table style='width:400px' cellpadding=4><tr><td id=a style='width:30%'>a</td>\
                  <td id=b>b</td><td id=c style='width:50px'>c</td></tr></table>";
    let (a, c) = (place(markup, "a").1, place(markup, "c").1);
    assert!(near(a, 117.6), "{a}");
    assert!(near(c, 58.0), "{c}");
}

/** `table-layout: fixed` sizes columns from the first row and ignores later content. */
#[test]
fn fixed_layout_uses_the_first_row() {
    let markup = "<table style='table-layout:fixed;width:300px'><tr><td id=c style='width:50px'>\
                  xxxxxxxxxxxxxxxxx</td><td id=d>y</td></tr></table>";
    assert!(near(place(markup, "c").1, 52.0), "{:?}", place(markup, "c"));
    assert!(
        near(place(markup, "d").1, 242.0),
        "{:?}",
        place(markup, "d")
    );
}

/** A forced line break ends a line, so a cell's max-content is its widest line. */
#[test]
fn line_breaks_narrow_the_max_content() {
    let markup = "<table border=1><tr><td id=a rowspan=2>tall<br>cell<br>here</td><td>r1</td></tr>\
                  <tr><td>r2</td></tr></table>";
    assert!(
        near(place(markup, "a").1, 36.02),
        "{:?}",
        place(markup, "a")
    );
}

/** A row box spans its cells inside the outer border spacing. */
#[test]
fn rows_sit_inside_the_border_spacing() {
    let markup = "<table border=1><tr id=r><td>one</td><td>two</td></tr></table>";
    assert!(near(place(markup, "r").0, 3.0), "{:?}", place(markup, "r"));
}
