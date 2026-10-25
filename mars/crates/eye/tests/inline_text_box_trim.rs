/** `text-box-trim` and `text-box-edge` after Blink's InlineLayoutAlgorithm::ApplyTextBoxTrim
and BlockLayoutAlgorithm::UpdateTextBoxTrim: the first formatted line's top and the last one's
bottom move to the root font's edges, through blocks without border or padding between. */
use mars_eye::{compute_styles, parse, render};

/** (y, height) of every element with an id, in document order, for `body` markup. */
fn boxes(body: &str) -> Vec<(String, f64, f64)> {
    let markup = format!("<body style='margin:0'>{body}");
    let document = compute_styles(&parse(&markup).unwrap(), &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    document
        .nodes
        .iter()
        .filter(|node| !node.html_id.is_empty())
        .filter_map(|node| {
            let item = display.items.iter().find(|i| i.local_id == node.local_id)?;
            Some((node.html_id.clone(), item.y, item.height))
        })
        .collect()
}
/** The (y, height) of `id`. */
fn at(body: &str, id: &str) -> (f64, f64) {
    let found = boxes(body);
    found
        .iter()
        .find(|(name, ..)| name == id)
        .map(|(_, y, h)| (*y, *h))
        .unwrap_or_else(|| panic!("{id} in {found:?}"))
}

/** Ahem at 20px/3: 20px of half-leading on each side of a 20px content area. */
const AHEM: &str = "font:20px/3 monospace";

/** Trimming both ends of a one-line block takes the half-leading off each side. */
#[test]
fn trim_both_cuts_half_leading() {
    let body = format!("<div id=t style='{AHEM};text-box:trim-both text'>X</div><div id=n></div>");
    let (_, height) = at(&body, "t");
    let (untrimmed_y, untrimmed) = at(&format!("<div id=t style='{AHEM}'>X</div>"), "t");
    assert_eq!(untrimmed_y, 0.0);
    assert!(height < untrimmed - 30.0, "{height} vs {untrimmed}");
}

/** The trim passes into the first and last child blocks and shrinks them too. */
#[test]
fn trim_propagates_into_child_blocks() {
    let body = format!(
        "<div id=p style='{AHEM};text-box-trim:trim-both;text-box-edge:text'>\
         <div id=a>X</div><div id=b>X</div></div>"
    );
    let (_, a) = at(&body, "a");
    let (_, b) = at(&body, "b");
    let (_, p) = at(&body, "p");
    assert!(a < 60.0 && b < 60.0, "{a} {b}");
    assert!((p - a - b).abs() < 0.01, "{p} {a} {b}");
}

/** Padding stops a trim from passing into a child. */
#[test]
fn padding_blocks_propagation() {
    let body = format!(
        "<div style='{AHEM};text-box:trim-both text'><div id=c style='padding:1px 0'>X</div></div>"
    );
    assert_eq!(at(&body, "c").1, 62.0);
}

/** An empty first block takes the first formatted line, so nothing after it trims. */
#[test]
fn empty_block_takes_the_first_line() {
    let body = format!(
        "<div style='{AHEM};text-box:trim-both text'><div></div><div id=a>X</div><div></div></div>"
    );
    assert_eq!(at(&body, "a").1, 60.0);
}

/** An empty inline box before a block stands for an anonymous block with no line. */
#[test]
fn empty_inline_box_takes_the_first_line() {
    let body = format!(
        "<div style='{AHEM};text-box:trim-both text'><span style='border:1px solid'> </span>\
         <div id=a>X</div><span style='border:1px solid'> </span></div>"
    );
    assert_eq!(at(&body, "a").1, 60.0);
}

/** `<br clear>` clearance after the last line keeps the block's end at the float's bottom. */
#[test]
fn clearance_after_the_last_line_is_kept() {
    let body = "<div style='float:left;width:10px;height:100px'></div>\
         <div id=t style='font:100px/1 monospace;text-box:trim-end text alphabetic'>X<br style='clear:both'></div>";
    assert_eq!(at(body, "t").1, 100.0);
}
