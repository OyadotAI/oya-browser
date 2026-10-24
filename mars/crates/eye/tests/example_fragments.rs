/** Fragment parsing and atomic change-log replay for example.com's frozen script. */
mod support;
use mars_eye::{
    DomChange, InsertPosition, apply_changes, constants::FRAGMENT_BYTES, constants::STATEMENTS,
    parse, parse_fragment, role, script_document,
};
use support::{EXAMPLE_HTML, applied, change_log, fragments, projected_digest};

/** The paragraph fragment yields five paragraphs and a link, numbered from first_id in preorder. */
#[test]
fn paragraph_fragment_parses_with_ids_from_first_id() {
    let fragment = parse_fragment("body", fragments().0, 1).unwrap();
    let shape = fragment
        .nodes
        .iter()
        .map(|node| (node.local_id, node.tag.as_str(), node.parent))
        .collect::<Vec<_>>();
    assert_eq!(
        shape,
        [
            (1, "p", None),
            (2, "p", None),
            (3, "p", None),
            (4, "p", None),
            (5, "p", None),
            (6, "a", None)
        ]
    );
    let langs = fragment.nodes[..5]
        .iter()
        .map(|node| node.attributes["lang"].as_str())
        .collect::<Vec<_>>();
    assert_eq!(langs, ["ar", "zh", "fr", "ru", "es"]);
    assert_eq!(fragment.nodes[0].attributes["dir"], "rtl");
    assert_eq!(
        fragment.nodes[5].href.as_deref(),
        Some("https://iana.org/help/example-domains")
    );
    assert_eq!(fragment.nodes[5].text, "Learn more");
    assert!(fragment.styles.is_empty());
}

/** The icon fragment yields its sheet, an inert style element and the svg subset in preorder. */
#[test]
fn icon_fragment_parses_style_and_svg_subset() {
    let fragment = parse_fragment("body", fragments().1, 7).unwrap();
    let shape = fragment
        .nodes
        .iter()
        .map(|node| (node.local_id, node.tag.as_str(), node.parent))
        .collect::<Vec<_>>();
    assert_eq!(
        shape,
        [
            (7, "style", None),
            (8, "svg", None),
            (9, "path", Some(8)),
            (10, "g", Some(8)),
            (11, "path", Some(10)),
            (12, "use", Some(10))
        ]
    );
    assert_eq!(fragment.styles.len(), 1);
    let svg = &fragment.nodes[1];
    assert_eq!(svg.attributes["viewbox"], "0,0,20,20");
    assert_eq!(svg.attributes["aria-hidden"], "true");
    assert_eq!(
        fragment.nodes[3].attributes["transform"],
        "rotate(-6,13.6,8.3)"
    );
    assert_eq!(fragment.nodes[4].html_id, "q");
    assert_eq!(fragment.nodes[5].attributes["x"], "2.4");
}

/** `use href` is an SVG reference, never a link target. */
#[test]
fn use_href_is_not_a_link() {
    let fragment = parse_fragment("body", fragments().1, 7).unwrap();
    let reference = fragment
        .nodes
        .iter()
        .find(|node| node.tag == "use")
        .unwrap();
    assert_eq!(reference.href, None);
    assert_ne!(role(reference), "link");
}

/** Page markup inserts and renders; scripts, handlers and unknown drawing input are inert, not refused. */
#[test]
fn inserted_page_markup_renders_inertly() {
    for html in [
        "<script>x()</script>",
        "<img src=a.png>",
        "<input value=x>",
        "<table><tr><td>x</td></tr></table>",
        "<iframe src=x></iframe>",
        "<svg><foreignObject></foreignObject></svg>",
        "<p onclick=x()>x</p>",
        "<path d='M0 0'/>",
        "<svg>text</svg>",
    ] {
        let fragment = parse_fragment("body", html, 1).unwrap();
        assert!(
            fragment.nodes.iter().all(|node| node.handler.is_none()),
            "{html}"
        );
        let mut document = parse("<p>host</p>").unwrap();
        document.nodes.extend(fragment.nodes);
        mars_eye::render(&mars_eye::compute_styles(&document, &[]).unwrap(), 640.0)
            .unwrap_or_else(|error| panic!("{html}: {}", error.message));
    }
}

/** Arc and cubic path data, now in Eye's grammar, render inside fragments. */
#[test]
fn fragment_arcs_and_cubics_render() {
    for html in [
        "<svg><path d='M0 0A1 1 0 0 1 2 2'/></svg>",
        "<svg><path d='M0 0C1 1 2 2 3 3'/></svg>",
    ] {
        let fragment = parse_fragment("body", html, 1).unwrap();
        let mut document = parse("<p>host</p>").unwrap();
        document.nodes.extend(fragment.nodes);
        let display =
            mars_eye::render(&mars_eye::compute_styles(&document, &[]).unwrap(), 640.0).unwrap();
        let svg = display.items.iter().find(|item| item.tag == "svg").unwrap();
        assert_eq!(svg.vector.len(), 1, "{html}");
    }
}

/** Raw-text contexts are refused before parsing. */
#[test]
fn unsupported_contexts_are_refused() {
    for context in ["script", "title", "textarea", "style", "iframe"] {
        assert_eq!(
            parse_fragment(context, "<p>x</p>", 1).unwrap_err().code,
            "unsupported_feature"
        );
    }
}

/** One fragment larger than FRAGMENT_BYTES fails explicitly. */
#[test]
fn fragment_bytes_limit_exceeded_fails_explicitly() {
    let html = format!("<p>{}</p>", "x".repeat(FRAGMENT_BYTES));
    let error = parse_fragment("body", &html, 1).unwrap_err();
    assert_eq!(error.code, "limit_exceeded");
    assert!(error.message.contains("fragment bytes"));
}

/** Fragment identities stay below NODES. */
#[test]
fn fragment_node_limit_exceeded_fails_explicitly() {
    let error =
        parse_fragment("body", "<p>a</p><p>b</p>", mars_eye::constants::NODES - 1).unwrap_err();
    assert_eq!(error.code, "limit_exceeded");
}

/** Replaying the log puts svg, the chosen paragraph, the rest and the link in order, with the style sheet added. */
#[test]
fn replayed_log_yields_icon_chosen_paragraph_rest_and_link() {
    let document = applied("fr");
    let order = document
        .nodes
        .iter()
        .filter(|node| node.parent.is_none())
        .map(|node| {
            (
                node.tag.as_str(),
                node.attributes
                    .get("lang")
                    .map(String::as_str)
                    .unwrap_or(""),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        order,
        [
            ("style", ""),
            ("svg", ""),
            ("p", "fr"),
            ("p", "en"),
            ("p", "ar"),
            ("p", "zh"),
            ("p", "ru"),
            ("p", "es"),
            ("a", "")
        ]
    );
    assert_eq!(document.styles.len(), 2);
    assert!(
        matches!(&document.styles[1], mars_eye::StyleSource::Inline(css) if css.starts_with("svg{"))
    );
}

/** Eye's applied document has exactly the structure digest V9's projection computes. */
#[test]
fn applied_document_matches_the_projection_digest() {
    let original = parse(EXAMPLE_HTML).unwrap();
    for language in ["en", "fr", "xx"] {
        let log = change_log(language);
        let applied = apply_changes(&original, &log).unwrap();
        assert_eq!(
            script_document(&applied, "example-page").unwrap().digest(),
            projected_digest(&original, &log)
        );
    }
}

/** A first_id below the high-water mark would reuse an identity and is invalid. */
#[test]
fn first_id_below_high_water_is_invalid() {
    let mut log = change_log("en");
    if let DomChange::InsertHtml { first_id, .. } = &mut log[3] {
        *first_id = 6;
    }
    let error = apply_changes(&parse(EXAMPLE_HTML).unwrap(), &log).unwrap_err();
    assert_eq!(error.code, "invalid_request");
}

/** Moving a node into its own subtree is invalid. */
#[test]
fn move_into_own_subtree_is_invalid() {
    let document = parse("<div id=a><p id=b>x</p></div>").unwrap();
    let error = apply_changes(
        &document,
        &[DomChange::Move {
            node: 0,
            target: Some(1),
            position: InsertPosition::BeforeEnd,
        }],
    )
    .unwrap_err();
    assert_eq!(error.code, "invalid_request");
}

/** Only lang and dir are reflected; href, id and handlers are refused. */
#[test]
fn set_attribute_outside_the_allowlist_is_refused() {
    let document = parse("<p>x</p><a href=/a>a</a>").unwrap();
    for name in ["href", "id", "onclick", "style"] {
        let error = apply_changes(
            &document,
            &[DomChange::SetAttribute {
                node: 1,
                name: name.into(),
                value: "x".into(),
            }],
        )
        .unwrap_err();
        assert_eq!(error.code, "unsupported_feature", "{name}");
    }
}

/** A failing change in the middle of the log leaves the input document untouched. */
#[test]
fn failing_change_mid_log_leaves_input_unchanged() {
    let original = parse(EXAMPLE_HTML).unwrap();
    let mut log = change_log("en");
    log.insert(
        2,
        DomChange::SetAttribute {
            node: 0,
            name: "href".into(),
            value: "https://attacker.invalid".into(),
        },
    );
    let before = serde_json::to_string(&original).unwrap();
    assert!(apply_changes(&original, &log).is_err());
    assert_eq!(serde_json::to_string(&original).unwrap(), before);
}

/** More changes than STATEMENTS fails explicitly. */
#[test]
fn change_count_limit_exceeded_fails_explicitly() {
    let document = parse("<p>x</p>").unwrap();
    let log = vec![
        DomChange::SetAttribute {
            node: 0,
            name: "lang".into(),
            value: "en".into(),
        };
        STATEMENTS + 1
    ];
    assert_eq!(
        apply_changes(&document, &log).unwrap_err().code,
        "limit_exceeded"
    );
}

/** Secret input values never reach the projected attributes. */
#[test]
fn projection_omits_secret_values() {
    let document = parse("<input type=password value=hunter2><input value=open>").unwrap();
    let projection = script_document(&document, "page").unwrap();
    assert!(!projection.nodes[0].attributes.contains_key("value"));
    assert_eq!(projection.nodes[1].attributes["value"], "open");
    assert_eq!(projection.nodes[0].tag, "input");
}

/** An over-long attribute name stays out of the script projection instead of failing the page. */
#[test]
fn overlong_attribute_names_do_not_break_projection() {
    let name = format!("data-{}", "a".repeat(mars_eye::constants::ID_BYTES));
    let document = mars_eye::parse(&format!("<p {name}=1>hello</p>")).unwrap();
    let projection = mars_eye::script_document(&document, "page").unwrap();
    assert!(
        projection
            .nodes
            .iter()
            .all(|node| !node.attributes.contains_key(&name))
    );
}
