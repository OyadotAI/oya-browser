/** Embedded content is never loaded or run by Eye: empty media show inert placeholder boxes, and
frames are replaced boxes the runtime fills with their documents. */
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

/** Video with fallback content renders the fallback; bare video gets a placeholder. */
#[test]
fn video_renders_fallback_or_placeholder() {
    let document = embeds();
    assert!(placeholder(&document, "clip").is_none());
    assert!(
        document
            .nodes
            .iter()
            .any(|n| n.text == "Download the video")
    );
    assert_eq!(
        placeholder(&document, "bare").unwrap().text,
        "Video (not supported yet)"
    );
}

/** Audio shows a box only with controls, as browsers size it. */
#[test]
fn audio_shows_a_box_only_with_controls() {
    let document = embeds();
    assert!(placeholder(&document, "podcast").is_some());
    assert!(placeholder(&document, "silent").is_none());
}

/** Canvas fallback text renders; an empty canvas gets the default 300 by 150 box. */
#[test]
fn canvas_renders_fallback_or_default_box() {
    let document = embeds();
    assert!(placeholder(&document, "chart").is_none());
    assert_eq!(by_id(&document, "chart").text, "A chart of downloads.");
    assert!(placeholder(&document, "blank").unwrap().attributes["style"].contains("width:300px"));
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
