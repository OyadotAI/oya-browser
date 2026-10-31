/** Data scripts read stays in the document without painting: comments inside elements become
`#comment` nodes holding their data as value, metadata elements and inert data blocks are kept
beside the tree for V9, and skipped foreign content never closes the element around it. */
use mars_eye::{Document, Node, compute_styles, parse, parse_fragment, render, script_document};

/** The first node with this author id. */
fn by_id<'a>(document: &'a Document, id: &str) -> &'a Node {
    document
        .nodes
        .iter()
        .find(|node| node.html_id == id)
        .unwrap_or_else(|| panic!("no #{id}"))
}

/** A comment inside an element is its child, carrying the comment data as its value. */
#[test]
fn element_comment_is_a_child_with_its_data() {
    let document = parse("<div><code id=c><!--{\"a\":1}--></code></div>").unwrap();
    let code = by_id(&document, "c");
    let comment = document
        .nodes
        .iter()
        .find(|node| node.tag == "#comment")
        .expect("a comment node");
    assert_eq!(comment.parent, Some(code.local_id));
    assert_eq!(comment.value, "{\"a\":1}");
    assert!(comment.text.is_empty(), "comment data is never text");
    let projection = script_document(&document, "page").unwrap();
    assert!(projection.nodes.iter().any(|node| node.tag == "#comment"));
}

/** Comment data never paints, even outside a hidden element. */
#[test]
fn comment_data_never_paints() {
    let document = parse("<p id=p>shown<!--secret config--></p>").unwrap();
    let display = render(&compute_styles(&document, &[]).unwrap(), 800.0).unwrap();
    let painted: String = display
        .items
        .iter()
        .flat_map(|item| item.lines.iter().map(|line| line.text.as_str()))
        .collect();
    assert!(painted.contains("shown"));
    assert!(!painted.contains("secret"), "{painted}");
    assert!(
        display.items.iter().all(|item| item.tag != "#comment"),
        "no comment box"
    );
}

/** A comment does not count as an element for structural selectors. */
#[test]
fn comment_is_not_an_element_sibling() {
    let document = parse("<ul><!--x--><li id=a>one</li></ul>").unwrap();
    let styled = compute_styles(&document, &["li:first-child{display:none}".into()]).unwrap();
    assert!(
        by_id(&styled, "a").hidden(),
        "the li is still the first child"
    );
}

/** Comments at the top level, in the head and directly under the body add no nodes; a comment
inside an element of an inserted fragment is a `#comment` node carrying its data, as the DOM
gives scripts. */
#[test]
fn comments_outside_elements_add_no_nodes_and_fragment_comments_are_kept() {
    let document =
        parse("<!--top--><html><head><!--h--></head><body><!--b--><p>x</p></body></html>").unwrap();
    assert!(!document.nodes.iter().any(|node| node.tag == "#comment"));
    let fragment = parse_fragment("div", "<span><!--f--></span>", 10).unwrap();
    let comments: Vec<_> = fragment
        .nodes
        .iter()
        .filter(|node| node.tag == "#comment")
        .collect();
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].value, "f");
    assert_eq!(comments[0].parent, Some(10));
    assert!(comments[0].text.is_empty());
}

/** Head metadata, a body link and data blocks are kept in source order with their text. */
#[test]
fn metadata_and_data_blocks_are_kept_in_order() {
    let document = parse(
        "<html><head><meta name=locale content=en_US><title>T</title><style>p{}</style></head>\
         <body><link id=ogb_ss rel=stylesheet media=print href=/x.css>\
         <script type=application/json id=d>{\"n\":2}</script><script>var ran=1</script></body></html>",
    )
    .unwrap();
    let tags: Vec<&str> = document.metadata.iter().map(|m| m.tag.as_str()).collect();
    assert_eq!(tags, ["meta", "title", "style", "link", "script"]);
    assert_eq!(document.metadata[0].attributes["content"], "en_US");
    assert_eq!(document.metadata[1].text, "T");
    assert_eq!(document.metadata[3].attributes["id"], "ogb_ss");
    assert_eq!(document.metadata[4].text, "{\"n\":2}");
    assert_eq!(document.scripts.len(), 1, "only the classic script runs");
    assert!(
        document
            .nodes
            .iter()
            .all(|node| !["meta", "link", "title"].contains(&node.tag.as_str()))
    );
}

/** Metadata keeps its handlers as authored, as a node does, for V9 to run on their events. */
#[test]
fn metadata_inline_handlers_are_kept() {
    let document = parse("<link id=l rel=preload href=/a onload=\"x()\">").unwrap();
    assert_eq!(document.metadata[0].attributes.get("onload").map(String::as_str), Some("x()"));
}

/** Script-inserted markup adds no metadata: V9 already holds what the script built. */
#[test]
fn fragments_keep_no_metadata() {
    let fragment = parse_fragment("div", "<link id=l rel=preload href=/a>", 10).unwrap();
    assert!(fragment.nodes.iter().all(|node| node.tag != "link"));
}

/** An element closed inside skipped foreign content closes there, not an open ancestor. */
#[test]
fn skipped_foreign_content_keeps_later_siblings_in_place() {
    let document =
        parse("<div id=a><svg><foreignObject><div></div></foreignObject></svg><p id=b></p></div>")
            .unwrap();
    assert_eq!(
        by_id(&document, "b").parent,
        Some(by_id(&document, "a").local_id)
    );
}
