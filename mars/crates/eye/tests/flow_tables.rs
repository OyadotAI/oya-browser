/** Table cell block sizing after Blink's layout/table: an authored cell height is a minimum, and
vertical alignment moves the content by its own height. */
use mars_eye::{compute_styles, parse, render};

/** (y, height) of `#id` after laying out `markup` at 800px with no body margin. */
fn place(markup: &str, id: &str) -> (f64, f64) {
    let markup = format!("<body style='margin:0'>{markup}");
    let document = compute_styles(&parse(&markup).unwrap(), &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    let local = document
        .nodes
        .iter()
        .find(|n| n.html_id == id)
        .unwrap()
        .local_id;
    let item = display.items.iter().find(|i| i.local_id == local).unwrap();
    (item.y, item.height)
}

/** Content taller than the authored height grows the cell and its row. */
#[test]
fn cell_height_is_a_minimum() {
    let markup = "<table cellspacing=0 cellpadding=0><tr><td id=c style='height:10px'><div style='height:40px'></div></td></tr></table>";
    assert_eq!(place(markup, "c").1, 40.0);
}

/** A middle-aligned cell centers its content by the content's height, even when the authored
height is smaller (Hacker News' header row). */
#[test]
fn middle_alignment_uses_the_content_height() {
    let markup = "<table cellspacing=0 cellpadding=0><tr><td><div style='height:40px;width:5px'></div></td>\
        <td style='height:10px'><div id=b style='height:20px'></div></td></tr></table>";
    assert_eq!(place(markup, "b").0, 10.0);
}

/** A taller authored height still centers the content inside it. */
#[test]
fn middle_alignment_inside_a_taller_authored_height() {
    let markup = "<table cellspacing=0 cellpadding=0><tr><td style='height:60px'><div id=b style='height:20px'></div></td></tr></table>";
    assert_eq!(place(markup, "b").0, 20.0);
}
