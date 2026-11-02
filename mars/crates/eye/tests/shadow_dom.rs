/** Shadow trees: declarative roots, slot flattening for layout, scoped styles and agent safety. */
use mars_eye::{
    Document, InsertPosition, Node, ScriptChange, apply_script_changes, compute_styles, flatten,
    parse, render,
};
use mars_eye::{DomChange, SHADOW_ROOT};

/** The node with `id`. */
fn by_id<'a>(document: &'a Document, id: &str) -> &'a Node {
    document
        .nodes
        .iter()
        .find(|node| node.html_id == id)
        .unwrap_or_else(|| panic!("no #{id}"))
}

/** Rendered text in paint order. */
fn painted(document: &Document) -> String {
    let display = render(document, 800.0).unwrap();
    display
        .items
        .iter()
        .flat_map(|item| item.lines.iter().map(|line| line.text.trim()))
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("|")
}

/** A host with a header, a named slot, a default slot and an unassigned child. */
const CARD: &str = "<x-card id=host><template shadowrootmode=open>\
    <style>p { color: red } :host { color: green } ::slotted(span) { color: blue }</style>\
    <p id=inner>Header</p><slot name=title></slot><div><slot></slot></div>\
    </template><span id=title slot=title>Title</span><span id=body>Body</span>\
    <b id=lost slot=missing>Lost</b></x-card><p id=outer>Outside</p>";

#[test]
fn declarative_shadow_roots_parse_as_a_root_node_of_their_host() {
    let document = parse(CARD).unwrap();
    let host = by_id(&document, "host").local_id;
    let root = document
        .nodes
        .iter()
        .find(|node| node.tag == SHADOW_ROOT)
        .expect("shadow root");
    assert_eq!(root.parent, Some(host));
    assert_eq!(
        root.attributes.get("mode").map(String::as_str),
        Some("open")
    );
    assert_eq!(by_id(&document, "inner").parent, Some(root.local_id));
    assert!(document.styles.is_empty(), "shadow styles stay scoped");
}

#[test]
fn layout_shows_shadow_content_with_slotted_children_in_their_slots() {
    let document = compute_styles(&parse(CARD).unwrap(), &[]).unwrap();
    assert_eq!(painted(&document), "Header|Title|Body|Outside");
    let flat = flatten(&document).unwrap();
    let title = by_id(&flat, "title");
    let slot = flat
        .nodes
        .iter()
        .find(|node| Some(node.local_id) == title.parent)
        .unwrap();
    assert_eq!(slot.tag, "slot");
    assert!(flat.nodes.iter().all(|node| node.tag != SHADOW_ROOT));
}

#[test]
fn unassigned_light_children_are_hidden_and_never_actionable() {
    let document = compute_styles(&parse(CARD).unwrap(), &[]).unwrap();
    let lost = by_id(&document, "lost");
    assert!(lost.hidden());
    assert!(!lost.style.interactive);
    assert!(!by_id(&document, "body").hidden());
}

#[test]
fn shadow_styles_apply_only_inside_their_tree() {
    let document = compute_styles(&parse(CARD).unwrap(), &[]).unwrap();
    assert_eq!(
        by_id(&document, "inner").style.color.as_deref(),
        Some("red")
    );
    assert_ne!(
        by_id(&document, "outer").style.color.as_deref(),
        Some("red")
    );
}

#[test]
fn host_and_slotted_rules_match_the_host_and_its_slotted_children() {
    let document = compute_styles(&parse(CARD).unwrap(), &[]).unwrap();
    assert_eq!(
        by_id(&document, "host").style.color.as_deref(),
        Some("green")
    );
    assert_eq!(
        by_id(&document, "title").style.color.as_deref(),
        Some("blue")
    );
}

#[test]
fn document_rules_do_not_reach_into_shadow_trees() {
    let document = compute_styles(
        &parse(CARD).unwrap(),
        &["p { font-weight: bold } span { color: purple }".into()],
    )
    .unwrap();
    assert!(!by_id(&document, "inner").style.bold);
    assert!(by_id(&document, "outer").style.bold);
    // Slotted light children are still the document's: its rules reach them.
    assert_eq!(
        by_id(&document, "body").style.color.as_deref(),
        Some("#800080")
    );
}

#[test]
fn scripts_attach_and_fill_a_shadow_root_through_the_change_log() {
    let document = parse("<div id=host><span id=light>Light</span></div>").unwrap();
    let host = by_id(&document, "host").local_id;
    let first = document.nodes.len() + 10;
    let attach = ScriptChange::Dom {
        change: DomChange::InsertHtml {
            target: Some(host),
            position: InsertPosition::AfterBegin,
            html: "<template shadowrootmode=\"open\"></template>".into(),
            first_id: first,
        },
    };
    let fill = ScriptChange::Dom {
        change: DomChange::InsertHtml {
            target: Some(first),
            position: InsertPosition::BeforeEnd,
            html: "<style>b { color: red }</style><b id=shadow>Shadow</b><slot></slot>".into(),
            first_id: first + 1,
        },
    };
    let changed = apply_script_changes(&document, &[attach, fill]).unwrap();
    assert!(
        changed.styles.is_empty(),
        "the inserted style stays in the shadow tree"
    );
    let styled = compute_styles(&changed, &[]).unwrap();
    assert_eq!(painted(&styled), "Shadow|Light");
    assert_eq!(by_id(&styled, "shadow").style.color.as_deref(), Some("red"));
}

#[test]
fn text_content_on_a_host_keeps_its_shadow_root() {
    let document = parse(CARD).unwrap();
    let host = by_id(&document, "host").local_id;
    let changed = apply_script_changes(
        &document,
        &[ScriptChange::NodeText {
            node: host,
            text: "Replaced".into(),
        }],
    )
    .unwrap();
    assert!(changed.nodes.iter().any(|node| node.tag == SHADOW_ROOT));
    assert!(changed.nodes.iter().all(|node| node.html_id != "title"));
}

#[test]
fn closed_roots_still_render_for_people_and_agents() {
    let markup = "<x-a><template shadowrootmode=closed><button id=b type=button>Go</button></template></x-a>";
    let document = compute_styles(&parse(markup).unwrap(), &[]).unwrap();
    assert_eq!(painted(&document), "Go");
    assert!(!by_id(&document, "b").hidden());
}
