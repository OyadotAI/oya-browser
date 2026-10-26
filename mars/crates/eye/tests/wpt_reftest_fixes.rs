/** Rules the WPT reftest scoreboard found Eye missing: the monospace font-size switch, auto-width
tables, preserved spaces in intrinsic widths and table parts inside inline boxes. */
use mars_eye::{DisplayList, Document, compute_styles, parse, render};

/** A styled document with no body margin. */
fn styled(markup: &str) -> Document {
    compute_styles(&parse(markup).unwrap(), &["body{margin:0}".to_owned()]).unwrap()
}

/** The computed font size of the element with this id. */
fn font_size(document: &Document, id: &str) -> f64 {
    document
        .nodes
        .iter()
        .find(|node| node.html_id == id)
        .unwrap()
        .style
        .font_size
}

/** The laid-out (x, y, width) of the element with this id. */
fn spot(document: &Document, display: &DisplayList, id: &str) -> (f64, f64, f64) {
    let local = document
        .nodes
        .iter()
        .find(|node| node.html_id == id)
        .unwrap()
        .local_id;
    let item = display.items.iter().find(|i| i.local_id == local).unwrap();
    (item.x, item.y, item.width)
}

/** `font-family: monospace` alone takes Chromium's 13px fixed default from the keyword size,
and a relative size scales from it; a second family or an absolute size opts out. */
#[test]
fn generic_monospace_takes_the_fixed_font_size() {
    let document = styled(
        "<div id=m style='font-family:monospace'>a<span id=e style='font-size:2em'>b</span>\
         <span id=s style='font-family:serif'>c</span></div>\
         <div id=two style='font-family:monospace,monospace'>d</div>\
         <div id=px style='font-family:monospace;font-size:16px'>e</div>\
         <pre id=pre>f<code id=code>g</code></pre>",
    );
    assert_eq!(font_size(&document, "m"), 13.0);
    assert_eq!(font_size(&document, "e"), 26.0);
    assert_eq!(
        font_size(&document, "s"),
        16.0,
        "back to the proportional keyword size"
    );
    assert_eq!(font_size(&document, "two"), 16.0);
    assert_eq!(font_size(&document, "px"), 16.0);
    assert_eq!(font_size(&document, "pre"), 13.0);
    assert_eq!(
        font_size(&document, "code"),
        13.0,
        "no second shrink inside pre"
    );
}

/** An absolutely positioned table with both insets set keeps its fit-content width instead of
stretching between them, as Blink sizes auto-width tables. */
#[test]
fn positioned_table_does_not_stretch_between_insets() {
    let document = styled(
        "<div style='position:relative'><table id=t style='position:absolute;left:1px;\
         right:1px;border-spacing:0'><tr><td style='padding:0'>\
         <div style='width:50px;height:10px'></div></td></tr></table></div>",
    );
    let display = render(&document, 800.0).unwrap();
    assert_eq!(spot(&document, &display, "t").2, 50.0);
}

/** A `white-space: pre` cell holding one space is a space wide: preserved white space counts in
max-content as line layout places it. */
#[test]
fn preserved_space_counts_in_max_content() {
    let document = styled(
        "<div style='display:block'><span id=a style='display:table-cell'>a</span>\
         <span id=sp style='display:table-cell;white-space:pre'> </span>\
         <span id=d style='display:table-cell'>d</span></div>",
    );
    let display = render(&document, 800.0).unwrap();
    let (space, d) = (
        spot(&document, &display, "sp"),
        spot(&document, &display, "d"),
    );
    assert!(space.2 > 0.0, "the space cell has width");
    assert_eq!(d.0, space.0 + space.2);
}

/** A table cell inside an inline box gets an anonymous inline-table, so it stays on the line
between its inline siblings instead of breaking it. */
#[test]
fn table_cell_in_an_inline_box_stays_on_the_line() {
    let document = styled(
        "<div><span><span id=a>a</span> <span id=b style='display:table-cell'>b</span>\
         <span id=c>c d</span></span></div>",
    );
    let display = render(&document, 800.0).unwrap();
    let (a, b, c) = (
        spot(&document, &display, "a"),
        spot(&document, &display, "b"),
        spot(&document, &display, "c"),
    );
    assert_eq!((a.1, b.1), (c.1, c.1), "one line");
    assert!(b.0 > a.0 + a.2, "the source space stays before the cell");
    assert_eq!(c.0, b.0 + b.2);
}

/** `display: inline-table` is an atomic inline: text before and after it shares its line. */
#[test]
fn inline_table_shares_its_line() {
    let document = styled(
        "<div>a <span id=t style='display:inline-table'><span style='display:table-cell'>b\
         </span></span><span id=c>c</span></div>",
    );
    let display = render(&document, 800.0).unwrap();
    let (t, c) = (
        spot(&document, &display, "t"),
        spot(&document, &display, "c"),
    );
    assert_eq!(t.1, c.1);
    assert_eq!(c.0, t.0 + t.2);
}

/** The laid-out height of the element with this id. */
fn height(document: &Document, display: &DisplayList, id: &str) -> f64 {
    let local = document
        .nodes
        .iter()
        .find(|node| node.html_id == id)
        .unwrap()
        .local_id;
    display
        .items
        .iter()
        .find(|i| i.local_id == local)
        .unwrap()
        .height
}

/** A vertical `-webkit-box` with `-webkit-line-clamp` ends after that many lines, its last kept
line ending in "…" and the rest hidden; the property alone does nothing. */
#[test]
fn webkit_line_clamp_ends_the_box_at_its_line() {
    let clamp = "font:16px/20px serif;white-space:pre";
    let document = styled(&format!(
        "<div id=c style='display:-webkit-box;-webkit-box-orient:vertical;\
         -webkit-line-clamp:2;{clamp}'>one\ntwo\nthree</div>\
         <div id=n style='-webkit-line-clamp:2;{clamp}'>one\ntwo\nthree</div>"
    ));
    let display = render(&document, 800.0).unwrap();
    assert_eq!(height(&document, &display, "c"), 40.0);
    assert_eq!(
        height(&document, &display, "n"),
        60.0,
        "no -webkit-box, no clamp"
    );
    let texts: Vec<String> = display
        .items
        .iter()
        .flat_map(|item| item.lines.iter())
        .map(|line| line.text.clone())
        .filter(|text| !text.is_empty())
        .collect();
    // The marker is its own run in the line's root style, after the clamp line's text.
    assert_eq!(texts[..3], ["one", "two", "\u{2026}"]);
}

/** A length past the geometry bound clamps to it instead of being dropped, so a 10000px block
stays tall. */
#[test]
fn huge_lengths_clamp_rather_than_drop() {
    let document =
        styled("<div id=t style='height:10000px'></div><div id=h style='height:999999px'></div>");
    let display = render(&document, 800.0).unwrap();
    assert_eq!(height(&document, &display, "t"), 10000.0);
    assert_eq!(
        height(&document, &display, "h"),
        mars_eye::constants::CSS_MAX_SCALAR
    );
}

/** An auto-width block with a definite height takes its width through `aspect-ratio`, and
`auto <ratio>` sizes the content box even under `box-sizing: border-box`. */
#[test]
fn aspect_ratio_transfers_height_to_width_and_auto_sizes_content() {
    let document = styled(
        "<div id=w style='height:100px;aspect-ratio:1/1'></div>\
         <div id=a style='width:100px;aspect-ratio:auto 1/1;box-sizing:border-box;\
         padding-left:50px'></div>",
    );
    let display = render(&document, 800.0).unwrap();
    assert_eq!(spot(&document, &display, "w").2, 100.0);
    assert_eq!(height(&document, &display, "a"), 50.0);
}

/** A height the ratio gives grows to fit taller content (the automatic minimum size). */
#[test]
fn ratio_height_grows_to_fit_content() {
    let document = styled(
        "<div id=r style='width:100px;aspect-ratio:2/1'><div style='height:80px'></div></div>",
    );
    let display = render(&document, 800.0).unwrap();
    assert_eq!(height(&document, &display, "r"), 80.0);
}

/** A styled document with its own sheet and no default body margin override. */
fn sheet(markup: &str, css: &str) -> Document {
    compute_styles(&parse(markup).unwrap(), &[css.to_owned()]).unwrap()
}

/** The root element's margin, border and padding frame the body, and its border is painted. */
#[test]
fn root_margin_border_and_padding_frame_the_body() {
    let document = sheet(
        "<p id=p style='margin:0'>x</p>",
        "html{margin:10px;border:20px solid black;padding:30px}body{margin:0}",
    );
    let display = render(&document, 800.0).unwrap();
    let p = spot(&document, &display, "p");
    assert_eq!((p.0, p.1, p.2), (60.0, 60.0, 680.0));
    let root = display.root_decoration.expect("root border");
    assert_eq!(
        (root.x, root.y, root.width, root.border),
        (10.0, 10.0, 780.0, 20.0)
    );
}

/** The body's box starts and ends where its first and last children's margins collapse through
it, and floats overflow it, so its background shows only around in-flow content. */
#[test]
fn body_box_follows_collapsed_margins_and_ignores_floats() {
    let document = sheet(
        "<p id=p style='margin:16px 0;height:20px'></p><div style='float:left;height:200px'></div>",
        "body{margin:0;background:red}html{background:white}",
    );
    let display = render(&document, 800.0).unwrap();
    let body = display.body_decoration.expect("body box");
    assert_eq!((body.y, body.height), (16.0, 20.0));
    assert!(
        display.height >= 236.0,
        "the float still extends the document"
    );
}

/** A definite body height sets the document's height. */
#[test]
fn definite_body_height_sets_the_document_height() {
    let document = sheet("<p>x</p>", "body{margin:0;height:10000px}");
    let display = render(&document, 800.0).unwrap();
    assert_eq!(display.height, 10000.0);
}

/** A `<col>` inside an open `<colgroup>` stays in it, as HTML's "in column group" mode keeps
it; clearing back to the table closed the group and shifted its columns. */
#[test]
fn col_stays_inside_its_colgroup() {
    let document = parse(
        "<table><colgroup span=2></colgroup><colgroup><col id=c></colgroup>\
         <tr><td></table>",
    )
    .unwrap();
    let col = document.nodes.iter().find(|n| n.html_id == "c").unwrap();
    let parent = col
        .parent
        .and_then(|p| document.nodes.iter().find(|n| n.local_id == p));
    assert_eq!(parent.unwrap().tag, "colgroup");
}

/** In a right-to-left table a spanning cell starts at its last (leftmost) column, so it stays
inside the table. */
#[test]
fn rtl_spanning_cell_stays_inside_its_table() {
    let document = styled(
        "<table id=t dir=rtl style='border-spacing:0'><tr><td id=s colspan=2 \
         style='padding:0'></td></tr><tr><td style='width:10px;padding:0'></td>\
         <td style='width:10px;padding:0'></td></tr></table>",
    );
    let display = render(&document, 800.0).unwrap();
    let (tx, _, tw) = spot(&document, &display, "t");
    let (sx, _, sw) = spot(&document, &display, "s");
    assert!(
        sx >= tx && sx + sw <= tx + tw + 0.01,
        "{sx}+{sw} in {tx}+{tw}"
    );
}
