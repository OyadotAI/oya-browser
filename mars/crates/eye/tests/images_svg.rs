/** An img showing an SVG drawing paints Eye's own vector paths, fitted to its box. */
use mars_eye::{DisplayItem, DisplayList, Document, PathCommand, compute_styles, parse, render};

/** Lays out one img whose source is `src`, with the intrinsic size the coordinator would set. */
fn layout(src: &str, size: (u32, u32), style: &str) -> (Document, DisplayList) {
    let mut document = parse(&format!("<img id=i alt=logo style='{style}'>")).unwrap();
    let img = document.nodes.iter_mut().find(|n| n.tag == "img").unwrap();
    img.image_src = Some(src.into());
    img.image_size = Some(size);
    let document = compute_styles(&document, &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    (document, display)
}

/** The item painted for the img. */
fn image<'a>(document: &Document, display: &'a DisplayList) -> &'a DisplayItem {
    let id = document
        .nodes
        .iter()
        .find(|n| n.tag == "img")
        .unwrap()
        .local_id;
    display
        .items
        .iter()
        .find(|item| item.local_id == id)
        .unwrap()
}

/** The drawing's viewBox scales uniformly into the img's CSS box. */
#[test]
fn svg_drawing_scales_into_the_image_box() {
    let src = "data:image/svg+xml,<svg viewBox=\"0 0 10 5\"><path fill=\"%23ff0000\" d=\"M0 0H10V5Z\"></path></svg>";
    let (document, display) = layout(src, (10, 5), "width:100px");
    let item = image(&document, &display);
    assert_eq!((item.width, item.height), (100.0, 50.0));
    let path = &item.vector[0];
    assert_eq!(path.fill.as_deref(), Some("#ff0000"));
    let PathCommand::Line { x, y } = path.commands[2] else {
        panic!("expected a line");
    };
    assert_eq!((x - item.x, y - item.y), (100.0, 50.0));
}

/** `currentColor` paints in the img's text color. */
#[test]
fn current_color_follows_the_image_color() {
    let src = "data:image/svg+xml,<svg viewBox=\"0 0 1 1\"><path fill=\"currentColor\" d=\"M0 0H1V1Z\"></path></svg>";
    let (document, display) = layout(src, (1, 1), "color:#00ff00;width:8px");
    assert_eq!(
        image(&document, &display).vector[0].fill.as_deref(),
        Some("#00ff00")
    );
}

/** Broken markup and bitmap sources paint no vector and never fail the page. */
#[test]
fn broken_or_bitmap_sources_paint_no_vector() {
    for src in [
        "data:image/svg+xml,%3C%ZZ<svg",
        "data:image/svg+xml,<svg><path d=\"M0 0 L nonsense\"></path></svg>",
        "https://example.test/a.png",
    ] {
        let (document, display) = layout(src, (4, 4), "");
        assert!(image(&document, &display).vector.is_empty(), "{src}");
    }
}

/** Basic shapes draw as their equivalent paths, in inline svg as in images. */
#[test]
fn basic_shapes_draw_as_paths() {
    let markup = "<svg id=s width=20 height=20><rect width=10 height=4 fill=red></rect><rect x=1 width=8 height=8 rx=2></rect><circle cx=10 cy=10 r=5></circle><ellipse cx=5 cy=5 rx=3 ry=2></ellipse><line x1=0 y1=0 x2=5 y2=5 stroke=black></line><polygon points='0,0 5,0 5,5'></polygon><polyline points='0 0 1 1' stroke=black></polyline><circle r=0></circle></svg>";
    let document = compute_styles(&parse(markup).unwrap(), &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    let svg = document
        .nodes
        .iter()
        .find(|n| n.tag == "svg")
        .unwrap()
        .local_id;
    let item = display.items.iter().find(|i| i.local_id == svg).unwrap();
    assert_eq!(item.vector.len(), 8);
    let rect = &item.vector[0].commands;
    assert_eq!(rect.len(), 5);
    let PathCommand::Line { x, y } = rect[2] else {
        panic!("expected a line");
    };
    assert_eq!((x - item.x, y - item.y), (10.0, 4.0));
    assert!(
        item.vector[2]
            .commands
            .iter()
            .any(|c| matches!(c, PathCommand::Cubic { .. }))
    );
    assert!(
        item.vector[7].commands.is_empty(),
        "a zero radius draws nothing"
    );
}

/** A malformed shape length drops that shape alone. */
#[test]
fn malformed_shape_lengths_drop_only_that_shape() {
    let markup = "<svg width=20 height=20><rect width=50% height=4></rect><circle cx=1 cy=1 r=1></circle></svg>";
    let document = compute_styles(&parse(markup).unwrap(), &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    let svg = document
        .nodes
        .iter()
        .find(|n| n.tag == "svg")
        .unwrap()
        .local_id;
    let item = display.items.iter().find(|i| i.local_id == svg).unwrap();
    assert_eq!(item.vector.len(), 1);
}

/** The box of the svg in `markup`, laid out at 800 points. */
fn svg_box(markup: &str) -> (f64, f64) {
    let document = compute_styles(&parse(markup).unwrap(), &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    let svg = document
        .nodes
        .iter()
        .find(|n| n.tag == "svg")
        .unwrap()
        .local_id;
    let item = display.items.iter().find(|i| i.local_id == svg).unwrap();
    (item.width, item.height)
}

/** Inline svg sizes like Chrome: CSS, then attributes, a missing side from the viewBox, then 300x150. */
#[test]
fn inline_svg_sizes_from_css_attributes_and_view_box() {
    let view = "viewBox='0 0 100 25'";
    assert_eq!(
        svg_box(&format!("<svg {view} style='height:26px'></svg>")),
        (104.0, 26.0)
    );
    assert_eq!(
        svg_box(&format!("<svg {view} style='width:40px'></svg>")),
        (40.0, 10.0)
    );
    assert_eq!(
        svg_box(&format!("<svg {view} width=20></svg>")),
        (20.0, 5.0)
    );
    assert_eq!(
        svg_box(&format!(
            "<div style='width:200px'><svg {view}></svg></div>"
        )),
        (200.0, 50.0)
    );
    assert_eq!(
        svg_box("<svg width=10 height=30 style='height:12px'></svg>"),
        (10.0, 12.0)
    );
    assert_eq!(svg_box("<svg></svg>"), (300.0, 150.0));
}
