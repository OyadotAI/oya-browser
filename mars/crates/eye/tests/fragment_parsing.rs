/** Script-inserted markup parses with the page parser's elements and fallbacks, never running anything. */
use mars_eye::{DomChange, InsertPosition, apply_changes, parse, parse_fragment};

/** Tags of a fragment's nodes, in preorder. */
fn tags(html: &str) -> Vec<String> {
    parse_fragment("body", html, 10)
        .unwrap()
        .nodes
        .into_iter()
        .map(|node| node.tag)
        .collect()
}

/** Images, controls, forms, tables, selects and media insert like page markup. */
#[test]
fn common_elements_are_accepted() {
    for html in [
        "<img src=a.png alt=x>",
        "<form action=/s><input name=q><button>Go</button></form>",
        "<label>a<input type=checkbox></label><textarea>t</textarea>",
        "<select><option>a<option selected>b</select>",
        "<table><tr><td>x</td></tr></table>",
        "<fieldset><legend>l</legend><output>o</output></fieldset>",
        "<picture><source srcset=a.webp><img src=a.png></picture>",
        "<template><p>x</p></template><noscript>n</noscript><math>m</math>",
        "<custom-card><span>x</span></custom-card>",
    ] {
        assert!(
            parse_fragment("body", html, 10).is_ok(),
            "{html} must parse"
        );
    }
}

/** Frames, media and canvases are replaced boxes the app fills: a script-built one parses to a
single node with no placeholder label, the structure the script engine holds. */
#[test]
fn embedded_content_parses_without_labels_or_extra_nodes() {
    let frame = parse_fragment("body", "<iframe src=https://ads.example/x></iframe>", 10).unwrap();
    assert_eq!(frame.nodes.len(), 1);
    assert!(frame.nodes[0].text.is_empty());
    // Media and canvases paint their own boxes: no label, no extra node.
    for (html, tag) in [
        ("<video src=v.mp4></video>", "video"),
        ("<audio controls src=a.mp3></audio>", "audio"),
        ("<canvas></canvas>", "canvas"),
    ] {
        let fragment = parse_fragment("body", html, 10).unwrap();
        assert_eq!(fragment.nodes.len(), 1, "{html}");
        assert_eq!(fragment.nodes[0].tag, tag);
        assert!(fragment.nodes[0].text.is_empty(), "{html}");
    }
    let text = mars_eye::DomFragments;
    let nodes = mars_core_parse(&text, "<iframe></iframe>");
    assert_eq!(nodes, 1, "the label never becomes a text child");
}

/** Node count DomFragments gives for `html` in a body. */
fn mars_core_parse(parser: &dyn mars_eye::FragmentParser, html: &str) -> usize {
    parser.parse("body", html, 10).unwrap().len()
}

/** Inserted scripts stay as empty inert elements and are never recorded to run. */
#[test]
fn inserted_scripts_never_run() {
    assert_eq!(
        tags("<p>a</p><script>steal()</script><p>b</p>"),
        ["p", "script", "p"]
    );
}

/** Inline handlers are kept for V9 to run, as browsers keep them; javascript: URLs stay inert. */
#[test]
fn handlers_are_kept_and_script_urls_inert() {
    let fragment = parse_fragment(
        "body",
        "<button onclick=x()>b</button><img src='javascript:x()' onerror=y()><a href='javascript:z()'>l</a>",
        10,
    )
    .unwrap();
    assert_eq!(fragment.nodes[0].handler.as_deref(), Some("x()"));
    assert_eq!(fragment.nodes[1].attributes.get("onerror").map(String::as_str), Some("y()"));
    assert!(fragment.nodes[1].image_src.is_none());
    assert!(fragment.nodes[2].href.is_none());
}

/** Document wrappers and metadata change nothing when inserted. */
#[test]
fn wrappers_and_metadata_are_ignored() {
    assert_eq!(
        tags(
            "<html><head><meta charset=latin1><base href=https://evil.example/><title>t</title></head><body><p>x</p></body></html>"
        ),
        ["p"]
    );
}

/** Rows insert into a table body, and options into a select. */
#[test]
fn table_and_select_contexts_are_accepted() {
    let rows = parse_fragment("tbody", "<tr><td>a</td><td>b</td></tr>", 10).unwrap();
    let shape: Vec<_> = rows
        .nodes
        .iter()
        .map(|node| (node.local_id, node.tag.as_str(), node.parent))
        .collect();
    assert_eq!(
        shape,
        [(10, "tr", None), (11, "td", Some(10)), (12, "td", Some(10))]
    );
    let options = parse_fragment("select", "<option>a</option><option>b</option>", 10).unwrap();
    assert_eq!(options.nodes.len(), 2);
}

/** Inside a drawing, the drawing subset parses and anything else is dropped like the page drops it. */
#[test]
fn svg_context_parses_the_drawing_subset() {
    let fragment = parse_fragment(
        "svg",
        "<path d='M0 0L1 1'/><text>t</text><g><rect/></g>",
        10,
    )
    .unwrap();
    let tags: Vec<_> = fragment
        .nodes
        .iter()
        .map(|node| node.tag.as_str())
        .collect();
    assert_eq!(tags, ["path", "g", "rect"]);
}

/** Raw-text contexts still refuse markup. */
#[test]
fn raw_text_contexts_are_refused() {
    for context in ["script", "style", "textarea", "iframe"] {
        assert_eq!(
            parse_fragment(context, "<p>x</p>", 1).unwrap_err().code,
            "unsupported_feature"
        );
    }
}

/** A form fragment is normalized like a page: a select with no selected option selects its first. */
#[test]
fn inserted_select_is_normalized() {
    let fragment = parse_fragment(
        "body",
        "<select><option>a</option><option>b</option></select>",
        40,
    )
    .unwrap();
    let selected: Vec<_> = fragment
        .nodes
        .iter()
        .filter(|n| n.tag == "option")
        .map(|n| n.selected)
        .collect();
    assert_eq!(selected, [true, false]);
    assert_eq!(fragment.nodes[0].local_id, 40);
}

/** An inserted image and iframe replay into a page with fresh identities above the page's. */
#[test]
fn inserted_media_replays_with_fresh_identities() {
    let document = parse("<div id=host>x</div>").unwrap();
    let first_id = document.nodes.len();
    let applied = apply_changes(
        &document,
        &[DomChange::InsertHtml {
            target: Some(0),
            position: InsertPosition::BeforeEnd,
            html: "<img src=a.png><iframe></iframe>".into(),
            first_id,
        }],
    )
    .unwrap();
    let inserted: Vec<_> = applied
        .nodes
        .iter()
        .filter(|n| n.local_id >= first_id)
        .collect();
    assert_eq!(inserted.len(), 2);
    assert_eq!(inserted[0].image_src.as_deref(), Some("a.png"));
}
