/** Embedded content is never loaded or run by Eye: objects and embeds show inert placeholder
boxes, while media, canvases and frames are replaced boxes the app fills itself. */
use mars_eye::{Document, Node, compute_styles, parse, render, role};

/** The embeds fixture, parsed once per test. */
fn embeds() -> Document {
    parse(include_str!("../../../fixtures/compat/embeds.html")).unwrap()
}

/** The first node with this author id. */
fn by_id<'a>(document: &'a Document, id: &str) -> &'a Node {
    document
        .nodes
        .iter()
        .find(|node| node.html_id == id)
        .unwrap_or_else(|| panic!("no #{id}"))
}

/** Direct children of a node. */
fn children<'a>(document: &'a Document, node: &Node) -> Vec<&'a Node> {
    document
        .nodes
        .iter()
        .filter(|child| child.parent == Some(node.local_id))
        .collect()
}

/** The placeholder child of an embedded element, if it has one. */
fn placeholder<'a>(document: &'a Document, id: &str) -> Option<&'a Node> {
    children(document, by_id(document, id))
        .into_iter()
        .find(|child| {
            child
                .attributes
                .get("aria-label")
                .is_some_and(|l| l.contains("not supported yet"))
        })
}

/** An iframe is a replaced box of its own, with no placeholder child; its raw content never
becomes nodes. */
#[test]
fn iframe_has_no_placeholder() {
    let document = embeds();
    assert!(placeholder(&document, "yt").is_none());
    assert!(children(&document, by_id(&document, "yt")).is_empty());
    assert!(!document.nodes.iter().any(|n| n.text.contains("inner text")));
}

/** Zero-size tracking frames get no placeholder. */
#[test]
fn zero_size_frames_get_no_placeholder() {
    assert!(placeholder(&embeds(), "pixel").is_none());
}

/** Video is a replaced box the app paints: no placeholder child, and its fallback content stays
in the tree for scripts but is hidden from rendering, as in a browser that plays video. */
#[test]
fn video_is_a_painted_box_without_placeholder() {
    let document = embeds();
    assert!(placeholder(&document, "clip").is_none());
    assert!(placeholder(&document, "bare").is_none());
    let styled = compute_styles(&document, &[]).unwrap();
    let fallback = styled
        .nodes
        .iter()
        .find(|n| n.text == "Download the video")
        .expect("fallback text stays in the tree");
    assert!(fallback.style.hidden, "fallback content must not render");
    assert!(!document.diagnostics.iter().any(|d| d.contains("Video")));
}

/** Audio shows a 300 by 54 control strip only with controls, as Chromium sizes its panel. */
#[test]
fn audio_shows_a_box_only_with_controls() {
    let document = embeds();
    let styled = compute_styles(&document, &[]).unwrap();
    assert!(placeholder(&document, "podcast").is_none());
    assert!(!by_id(&styled, "podcast").style.hidden);
    assert!(by_id(&styled, "silent").style.hidden);
    let display = render(&styled, 800.0).unwrap();
    let strip = display.items.iter().find(|item| item.local_id == by_id(&styled, "podcast").local_id).expect("audio box");
    assert_eq!((strip.width, strip.height), (300.0, 54.0));
}

/** Canvas fallback text stays in the tree; an empty canvas paints its own default 300 by 150
box (the app draws its bitmap there), with no placeholder child. */
#[test]
fn canvas_renders_fallback_or_default_box() {
    let document = embeds();
    assert!(placeholder(&document, "chart").is_none());
    assert_eq!(by_id(&document, "chart").text, "A chart of downloads.");
    assert!(placeholder(&document, "blank").is_none());
    let styled = compute_styles(&document, &[]).unwrap();
    let display = render(&styled, 800.0).unwrap();
    let blank = by_id(&styled, "blank").local_id;
    let item = display.items.iter().find(|item| item.local_id == blank).expect("canvas box");
    assert_eq!((item.width, item.height), (300.0, 150.0));
}

/** Objects render fallback; void embeds get a placeholder; picture keeps its image. */
#[test]
fn objects_embeds_and_pictures_fall_back() {
    let document = embeds();
    assert!(document.nodes.iter().any(|n| n.text == "Read the PDF"));
    let embed = document.nodes.iter().find(|n| n.tag == "embed").unwrap();
    assert_eq!(children(&document, embed).len(), 1);
    let image = document
        .nodes
        .iter()
        .find(|n| n.tag == "img" && n.image_alt == "Hero image")
        .unwrap();
    assert_eq!(document.nodes[image.parent.unwrap()].tag, "picture");
    assert!(document.nodes.iter().any(|n| n.text == "After the media."));
}

/** Placeholders are never actionable: no link, handler, or control semantics. */
#[test]
fn placeholders_are_not_actionable() {
    let document = embeds();
    for node in document
        .nodes
        .iter()
        .filter(|n| n.text.contains("not supported yet"))
    {
        assert!(node.href.is_none() && node.handler.is_none());
        assert_eq!(node.tag, "div");
        assert_eq!(role(node), "group");
    }
}

/** A frame reaches the display list at its author size inside html.css's 2px inset border. */
#[test]
fn frame_is_painted_at_its_size() {
    let document =
        parse("<iframe src=https://example.com/embed width=400 height=200></iframe><p>after</p>")
            .unwrap();
    let display = render(&compute_styles(&document, &[]).unwrap(), 800.0).unwrap();
    let item = display
        .items
        .iter()
        .find(|item| item.tag == "iframe")
        .unwrap();
    assert_eq!((item.width, item.height), (404.0, 204.0));
    assert!(item.lines.is_empty());
}

/** Embedded content inside a hidden ancestor stays hidden with it. */
#[test]
fn hidden_embeds_stay_hidden() {
    let document = parse("<div hidden><iframe src=x></iframe></div><p>shown</p>").unwrap();
    let styled = compute_styles(&document, &[]).unwrap();
    let frame = styled.nodes.iter().find(|n| n.tag == "iframe").unwrap();
    assert!(frame.hidden());
}
