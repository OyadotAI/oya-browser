/** Embedded documents: frames lay out as replaced boxes with CSS 2.1's 300 by 150 default, a
frame's document lays out in its content box as its viewport, and its display splices into the
embedder's under the frame's clip, layer and an identity range of its own. */
use mars_eye::{
    DisplayItem, DisplayList, Document, RenderContext, compute_frame_styles, compute_styles,
    embed_frame, frame_viewport, parse, render, render_frame,
};

/** The page's styled display at the WPT width. */
fn display(html: &str) -> (Document, DisplayList) {
    let styled = compute_styles(&parse(html).unwrap(), &[]).unwrap();
    let display = render(&styled, 800.0).unwrap();
    (styled, display)
}

/** The first item painted for this tag. */
fn item<'a>(display: &'a DisplayList, tag: &str) -> &'a DisplayItem {
    display
        .items
        .iter()
        .find(|item| item.tag == tag)
        .unwrap_or_else(|| panic!("no {tag}"))
}

/** A frame viewport of this size. */
fn viewport(width: f64, height: f64) -> RenderContext {
    RenderContext {
        width,
        height,
        ..RenderContext::default()
    }
}

/** An iframe with no size of its own is 300 by 150 inside html.css's 2px inset border. */
#[test]
fn iframe_takes_the_default_size_and_border() {
    let (_, display) = display("<body style=margin:0><iframe></iframe>");
    let frame = item(&display, "iframe");
    assert_eq!((frame.width, frame.height), (304.0, 154.0));
    assert_eq!(frame.border, 2.0);
    assert!(frame.lines.is_empty());
}

/** A frame has no natural ratio: an authored height leaves the width at 300 (CSS 2.1 10.3.2). */
#[test]
fn frame_height_does_not_scale_its_width() {
    let (_, display) =
        display("<body style=margin:0><iframe style='border:0;display:block;height:96px'>");
    let frame = item(&display, "iframe");
    assert_eq!((frame.width, frame.height), (300.0, 96.0));
}

/** An object the runtime embedded a document into is replaced and hides its fallback; one it
did not shows the fallback. */
#[test]
fn embedded_object_is_replaced() {
    let html = "<body style=margin:0><object data=doc.html type=text/html>Fallback</object>";
    let (styled, fallback) = display(html);
    assert!(fallback.items.iter().any(|item| !item.lines.is_empty()));
    let mut document = styled;
    let object = document
        .nodes
        .iter_mut()
        .find(|n| n.tag == "object")
        .unwrap();
    object.embedded = true;
    let shown = render(&document, 800.0).unwrap();
    let frame = item(&shown, "object");
    assert_eq!((frame.width, frame.height), (300.0, 150.0));
    assert!(shown.items.iter().all(|item| item.lines.is_empty()));
}

/** A frame's viewport is its content box, inside border and padding. */
#[test]
fn frame_viewport_is_the_content_box() {
    let (_, display) = display("<body style=margin:0><iframe style='padding:5px'>");
    assert_eq!(
        frame_viewport(item(&display, "iframe")),
        [7.0, 7.0, 300.0, 150.0]
    );
}

/** A frame's document styles and lays out at a viewport smaller than any window, and its
viewport units resolve against the frame. */
#[test]
fn frame_documents_lay_out_in_the_frame_viewport() {
    let child = parse("<body style=margin:0><div style='width:50vw;height:100vh'></div>").unwrap();
    assert!(compute_styles(&child, &[]).is_ok());
    let context = viewport(200.0, 40.0);
    let styled = compute_frame_styles(&child, &[], context).unwrap();
    let laid = render_frame(&styled, context).unwrap();
    let block = item(&laid, "div");
    assert_eq!((block.width, block.height), (100.0, 40.0));
    assert!(render_frame(&styled, viewport(0.0, 40.0)).is_err());
    assert!(mars_eye::render_scrolled(&styled, context, &Default::default()).is_err());
}

/** Splicing moves the child's boxes to the content box, clips them to it, gives them the
frame's layer and fresh identities, and paints no canvas for a document without a background. */
#[test]
fn embedding_moves_clips_and_renumbers_child_boxes() {
    let (_, mut page) =
        display("<body style=margin:0><p>a</p><iframe style='z-index:2;position:relative'>");
    let child = parse("<body style=margin:0><div style='height:500px'></div>").unwrap();
    let context = viewport(300.0, 150.0);
    let styled = compute_frame_styles(&child, &[], context).unwrap();
    let laid = render_frame(&styled, context).unwrap();
    let at = page
        .items
        .iter()
        .position(|item| item.tag == "iframe")
        .unwrap();
    let owner = page.items[at].clone();
    let before = page.items.len();
    embed_frame(&mut page, at, laid.clone(), &styled, 1 << 32);
    assert_eq!(page.items.len(), before + laid.items.len());
    let block = &page.items[at + 1];
    assert_eq!(block.tag, "div");
    assert!(block.local_id >= 1 << 32);
    assert_eq!((block.x, block.y), (owner.x + 2.0, owner.y + 2.0));
    let clip = block.clip.unwrap();
    assert_eq!((clip.width, clip.height), (300.0, 150.0));
    assert_eq!(block.z, owner.z);
}

/** A child root or body background paints the frame's canvas, under its content. */
#[test]
fn embedded_canvas_takes_the_child_background() {
    let (_, mut page) = display("<body style=margin:0><iframe>");
    let child = parse("<body style='background:green'>").unwrap();
    let context = viewport(300.0, 150.0);
    let styled = compute_frame_styles(&child, &[], context).unwrap();
    let laid = render_frame(&styled, context).unwrap();
    let at = page
        .items
        .iter()
        .position(|item| item.tag == "iframe")
        .unwrap();
    embed_frame(&mut page, at, laid, &styled, 1 << 32);
    let canvas = &page.items[at + 1];
    assert!(
        canvas
            .background
            .as_deref()
            .is_some_and(|c| c != "transparent")
    );
    assert_eq!((canvas.x, canvas.y, canvas.width), (2.0, 2.0, 300.0));
}
