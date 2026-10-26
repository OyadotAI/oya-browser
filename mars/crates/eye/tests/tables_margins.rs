/** Margin struts, `margin-trim`, white space before preserved text, XML `<pre>` newlines and the
web fonts a font-measuring page loads, as the display list and the font requests show them. */
use mars_eye::{DisplayList, Document, compute_styles, font_requests, parse, parse_xml, render};

/** A styled document, its inline `<style>` sheets applied, and its display list at 800px. */
fn layout(document: Document) -> (Document, DisplayList) {
    let sheets: Vec<String> = document
        .styles
        .iter()
        .filter_map(|source| match source {
            mars_eye::StyleSource::Inline(css) => Some(css.clone()),
            _ => None,
        })
        .collect();
    let document = compute_styles(&document, &sheets).unwrap();
    let display = render(&document, 800.0).unwrap();
    (document, display)
}
/** The first item of the element with `id`: (x, y, width, height). */
fn rect(document: &Document, display: &DisplayList, id: &str) -> (f64, f64, f64, f64) {
    let local = document
        .nodes
        .iter()
        .find(|n| n.html_id == id)
        .unwrap()
        .local_id;
    let item = display.items.iter().find(|i| i.local_id == local).unwrap();
    (item.x, item.y, item.width, item.height)
}

/** A strut adjoins its largest positive and most negative margins: 2, -4, 0, 14, -4 and 2px
collapse to 10px (abspos-022), not a pairwise sum. */
#[test]
fn margins_through_empty_blocks_collapse_as_one_strut() {
    let (document, display) = layout(
        parse(
            "<body style='margin:0'><div style='margin:2px'><div style='margin:-4px 0'>\
             <div style='margin:0 0 14px'></div></div></div><div id=a style='height:10px'></div>",
        )
        .unwrap(),
    );
    assert_eq!(rect(&document, &display, "a").1, 10.0);
}

/** An empty new formatting context ends the strut: the margin before it does not collapse with
the one after it (float-after-block-with-collapsed-margin-inside-inline). */
#[test]
fn empty_formatting_context_stops_a_strut() {
    let (document, display) = layout(
        parse(
            "<body style='margin:0'><div style='margin-bottom:16px;height:1px'></div>\
             <div style='overflow:hidden'></div><div id=a style='margin-top:-10px;height:5px'></div>",
        )
        .unwrap(),
    );
    assert_eq!(rect(&document, &display, "a").1, 7.0);
}

/** `margin-trim: block-end` drops the last child's bottom margin from its container's height,
and `block-start` the first child's top margin. */
#[test]
fn margin_trim_drops_edge_margins() {
    let (document, display) = layout(
        parse(
            "<body style='margin:0'><div id=e style='margin-trim:block-end;padding:1px'>\
             <div style='height:10px;margin-bottom:50px'></div></div>\
             <div id=s style='margin-trim:block-start;padding:1px'>\
             <div id=c style='height:10px;margin-top:50px'></div></div>",
        )
        .unwrap(),
    );
    assert_eq!(rect(&document, &display, "e").3, 12.0);
    let (container, child) = (
        rect(&document, &display, "s"),
        rect(&document, &display, "c"),
    );
    assert_eq!(child.1 - container.1, 1.0);
}

/** A collapsible space before preserved text stays: only a collapsible space removes one
(white-space-processing-031). */
#[test]
fn collapsible_space_stays_before_preserved_text() {
    let (document, display) = layout(
        parse(
            "<body style='margin:0;font:16px monospace'><span>X </span>\
             <span id=p style='white-space:pre'> X</span>",
        )
        .unwrap(),
    );
    let local = document
        .nodes
        .iter()
        .find(|n| n.html_id == "p")
        .unwrap()
        .local_id;
    let item = display.items.iter().find(|i| i.local_id == local).unwrap();
    // "X" then the kept space: the preserved run starts two monospace advances in.
    let x = item.lines[0].x;
    assert!(x > 15.0, "{x}");
}

/** XML keeps the newline right after `<pre>`, which only HTML drops (white-space-pre-001). */
#[test]
fn xml_pre_keeps_its_leading_newline() {
    let markup =
        "<html xmlns='http://www.w3.org/1999/xhtml'><body><pre id='p'>\nX</pre></body></html>";
    let html = parse("<pre id=p>\nX</pre>").unwrap();
    let xml = parse_xml(markup, None).unwrap();
    let text = |document: &Document| {
        let node = document.nodes.iter().find(|n| n.html_id == "p").unwrap();
        node.text.clone()
    };
    assert_eq!(text(&html), "X");
    assert_eq!(text(&xml), "\nX");
}

/** A page whose lengths measure its fonts loads them even for boxes without text
(border-top-width-080's empty Ahem div). */
#[test]
fn measuring_page_requests_fonts_of_empty_boxes() {
    let css = "@font-face { font-family: Probe; src: url(https://fonts.example/probe.woff2) } \
               div { font-family: Probe; border-top: 1ex solid }";
    let document = compute_styles(&parse("<div></div>").unwrap(), &[css.to_string()]).unwrap();
    assert_eq!(font_requests(&document).len(), 1);
}

/** The space between top-level inline-blocks is the body's text, in the body's font, not the
boxes' (border-top-width-072's 20px Ahem boxes, 4px apart). */
#[test]
fn top_level_space_takes_the_body_font() {
    let (document, display) = layout(
        parse(
            "<body style='margin:0;font:16px serif'><div style='display:inline-block;width:10px;\
             font-size:80px'></div> <div id=b style='display:inline-block;width:10px;font-size:80px'></div>",
        )
        .unwrap(),
    );
    let x = rect(&document, &display, "b").0;
    assert!(x < 20.0, "{x}");
}

/** An inline-block starting a line goes past a lower float its height reaches
(floats-wrap-top-below-inline-001l). */
#[test]
fn line_start_passes_floats_the_box_reaches() {
    let (document, display) = layout(
        parse(
            "<body style='margin:0'><div style='width:400px'>\
             <div style='float:left;clear:left;width:50px;height:75px'></div>\
             <div style='float:left;clear:left;width:100px;height:75px'></div>\
             <span style='display:inline-block;width:200px;height:50px'></span>\
             <span id=s style='display:inline-block;width:200px;height:50px'></span></div>",
        )
        .unwrap(),
    );
    assert_eq!(rect(&document, &display, "s").0, 100.0);
}
