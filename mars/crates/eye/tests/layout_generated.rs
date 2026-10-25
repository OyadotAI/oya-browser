/** Generated content, list markers, legacy presentational attributes and background image addresses. */
use mars_eye::{
    DisplayBackground, DisplayItem, DisplayList, Document, ImageSize, StyleSource, background_urls,
    compute_styles, parse, rebase_urls, render,
};

/** Styles from the document's own `<style>` elements, then the display list. */
fn layout(markup: &str) -> (Document, DisplayList) {
    let parsed = parse(markup).unwrap();
    let sheets = parsed
        .styles
        .iter()
        .filter_map(|source| match source {
            StyleSource::Inline(css) => Some(css.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let document = compute_styles(&parsed, &sheets).unwrap();
    let display = render(&document, 800.0).unwrap();
    (document, display)
}
/** Every item painted for the element with `id`, its own box first. */
fn items<'a>(document: &Document, display: &'a DisplayList, id: &str) -> Vec<&'a DisplayItem> {
    let local = document
        .nodes
        .iter()
        .find(|node| node.html_id == id)
        .unwrap()
        .local_id;
    display
        .items
        .iter()
        .filter(|item| item.local_id == local)
        .collect()
}

/** ::before text paints before the element's own text, in its own color. */
#[test]
fn before_content_paints_first() {
    let (document, display) = layout(
        "<style>p::before{content:'\\2192  ' attr(data-n);color:red}</style><p id=p data-n=7>text</p>",
    );
    let painted = items(&document, &display, "p");
    let before = painted.iter().find(|item| item.tag == "::before").unwrap();
    assert_eq!(before.lines[0].text, "\u{2192} 7");
    assert_eq!(before.color.as_deref(), Some("red"));
    assert!(before.lines[0].x < painted[0].lines[0].x);
}

/** A clearfix ::after grows its container around floats. */
#[test]
fn clearfix_after_contains_floats() {
    let (document, display) = layout(
        "<style>.c::after{content:'';display:table;clear:both}</style>\
         <div id=c class=c><div style='float:left;width:10px;height:70px'></div></div>",
    );
    assert!((items(&document, &display, "c")[0].height - 70.0).abs() < 0.01);
}

/** Icon-font code points are dropped rather than painted as missing glyphs. */
#[test]
fn private_use_content_is_dropped() {
    let (document, _) = layout("<style>i::before{content:'\\f101'}</style><i id=i></i>");
    let node = document.nodes.iter().find(|n| n.html_id == "i").unwrap();
    assert_eq!(node.style.generated[0].content, "");
}

/** Unordered items get bullets and ordered items get numbers, left of their content. */
#[test]
fn list_items_get_markers() {
    let (document, display) =
        layout("<ul><li id=a>one</li></ul><ol start=3><li>x</li><li id=b>two</li></ol>");
    let a = items(&document, &display, "a")[0];
    let bullet = a
        .lines
        .iter()
        .find(|line| line.text.starts_with('\u{2022}'))
        .unwrap();
    assert!(bullet.x < a.x);
    let b = items(&document, &display, "b")[0];
    assert!(b.lines.iter().any(|line| line.text == "4. "));
}

/** list-style: none removes the marker. */
#[test]
fn list_style_none_has_no_marker() {
    let (document, display) = layout("<ul style='list-style:none'><li id=a>one</li></ul>");
    assert_eq!(items(&document, &display, "a")[0].lines.len(), 1);
}

/** `<font color size>` and body `bgcolor` style like CSS. */
#[test]
fn legacy_attributes_style_like_css() {
    let (document, _) = layout("<p><font id=f color=#00ff00 size=5>x</font></p>");
    let font = &document
        .nodes
        .iter()
        .find(|n| n.html_id == "f")
        .unwrap()
        .style;
    assert_eq!(font.color.as_deref(), Some("#00ff00"));
    assert_eq!(font.font_size, 24.0);
}

/** Linked sheet URLs rebase onto the sheet; data and absolute URLs stay. */
#[test]
fn sheet_urls_rebase_to_the_sheet() {
    let css = "a{background:url(img/x.png)} b{background:url('data:image/png;base64,AA')} c{background:url(https://o.example/y.png)}";
    let out = rebase_urls(css, "https://site.example/css/main.css");
    assert!(out.contains("\"https://site.example/css/img/x.png\""));
    assert!(out.contains("data:image/png;base64,AA"));
    assert!(out.contains("https://o.example/y.png"));
}

/** Background images are listed per element and painted with their size, position and tiling. */
#[test]
fn background_images_are_listed_and_described() {
    let (document, display) = layout(
        "<div id=d style='background:url(/bg.png) no-repeat center / cover;height:40px'>x</div>",
    );
    let urls = background_urls(&document, "https://site.example/page");
    let local = document
        .nodes
        .iter()
        .find(|n| n.html_id == "d")
        .unwrap()
        .local_id;
    assert_eq!(urls, [(local, "https://site.example/bg.png".to_string())]);
    let item = items(&document, &display, "d")[0];
    let Some(DisplayBackground::Image {
        size,
        position,
        repeat,
        ..
    }) = item.backgrounds.first()
    else {
        panic!("expected an image layer");
    };
    assert_eq!(*size, ImageSize::Cover);
    assert_eq!(*position, [0.0, 50.0, 0.0, 50.0]);
    assert_eq!(*repeat, [false, false]);
}

/** An absolutely positioned ::after fills its positioned element's padding box from its insets. */
#[test]
fn absolute_after_is_placed_in_its_element() {
    let (document, display) = layout(
        "<style>.hr{position:relative;height:4px;width:300px}\
         .hr::after{content:'';position:absolute;left:0;right:0;top:0;height:4px;background:red}</style>\
         <div id=h class=hr></div>",
    );
    let painted = items(&document, &display, "h");
    let after = painted.iter().find(|item| item.tag == "::after").unwrap();
    assert_eq!(after.background.as_deref(), Some("red"));
    assert!((after.width - 300.0).abs() < 0.01 && (after.height - 4.0).abs() < 0.01);
    assert!((after.x - painted[0].x).abs() < 0.01);
}

/** An absolute ::after resolves percentage insets against its element's padding box and applies
its own transform: CNN's ad placeholder (`left:50%; transform:translate(-50%,-50%)`) centers on
the slot instead of sitting at its left edge. */
#[test]
fn absolute_after_resolves_percent_insets_and_its_own_transform() {
    let (document, display) = layout(
        "<style>.slot{position:relative;height:0;width:1000px}\
         .slot::after{content:'';display:block;position:absolute;width:728px;height:90px;\
         top:0;left:50%;transform:translate(-50%,-50%);background:#262626}</style>\
         <div id=s class=slot></div>",
    );
    let painted = items(&document, &display, "s");
    let after = painted.iter().find(|item| item.tag == "::after").unwrap();
    assert_eq!(after.transform, None, "a pure translation moves the box");
    assert!(
        (after.x - (painted[0].x + 136.0)).abs() < 0.01,
        "x {}",
        after.x
    );
    assert!(
        (after.y - (painted[0].y - 45.0)).abs() < 0.01,
        "y {}",
        after.y
    );
    assert!(
        (painted[0].x - painted[0].x).abs() < 0.01,
        "the element itself does not move"
    );
}

/** `rem` follows the root element's font size. */
#[test]
fn rem_follows_the_root_font_size() {
    let (document, _) =
        layout("<style>html{font-size:62.5%} p{font-size:1.6rem}</style><p id=p>x</p>");
    let p = document.nodes.iter().find(|n| n.html_id == "p").unwrap();
    assert!((p.style.font_size - 16.0).abs() < 0.01);
}

/** A url() mask-image icon keeps its fill and carries the mask that shapes it, so painters fill
 * only where the fetched icon is opaque; it stays out of agent reach as before. */
#[test]
fn masked_icons_paint_their_fill_through_the_mask() {
    let (document, display) = layout(
        "<style>.i{display:block;width:20px;height:20px;background-color:#000;mask-image:url(i.svg)}</style>\
         <span id=i class=i></span>",
    );
    let item = items(&document, &display, "i")[0];
    assert_eq!(item.background.as_deref(), Some("#000"));
    assert!(
        matches!(&item.mask, Some(DisplayBackground::Image { url, .. }) if url.ends_with("i.svg")),
        "{:?}",
        item.mask
    );
    assert!(!document.nodes[0].style.interactive);
}
