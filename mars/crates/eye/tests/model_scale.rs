/** Projection, labels, tables, text and CSS rules hold their limits and stay linear at the node limit. */
use mars_eye::{
    compute_styles,
    constants::{
        CSS_RULES, NODES, RPC_TIMEOUT, STATIC_TEXT_NODE_BYTES, STATIC_TEXT_TOTAL_BYTES, TABLE_ROWS,
        TEXT_BYTES,
    },
    label, parse, script_document,
};
use std::time::{Duration, Instant};

/** A third of the private call deadline, so one stage leaves room for the others in the same call. */
const STAGE_BUDGET: Duration = Duration::from_secs(RPC_TIMEOUT.as_secs() / 3);
/** Labeled inputs in the large fixture, all of them controls the node budget already bounds. */
const LABELED: usize = 120;

/** A NODES-node page with labeled inputs, wrapping labels and duplicate author IDs. */
fn large_page() -> String {
    let mut html = String::new();
    for index in 0..LABELED {
        html.push_str(&format!(
            "<label for=\"i{index}\">Field {index}</label><input id=\"i{index}\">"
        ));
    }
    let filler = NODES - 2 * LABELED;
    html.push_str(&"<p id=\"same\">text</p>".repeat(filler));
    html
}

/** Projection is linear at the node limit and still marks only the first holder of an ID writable. */
#[test]
fn projection_at_the_node_limit_is_linear_and_first_id_wins() {
    let document = parse(&large_page()).unwrap();
    assert_eq!(document.nodes.len(), NODES);
    let started = Instant::now();
    let projection = script_document(&document, "page").unwrap();
    assert!(started.elapsed() < STAGE_BUDGET, "{:?}", started.elapsed());
    let same = projection
        .nodes
        .iter()
        .filter(|node| node.html_id == "same")
        .map(|node| node.text_writable)
        .collect::<Vec<_>>();
    assert!(same[0]);
    assert!(same[1..].iter().all(|writable| !writable));
}

/** A small page with duplicate IDs projects exactly as the per-node first-match rule says. */
#[test]
fn projection_matches_the_naive_first_id_rule() {
    let document = parse("<p id=\"d\">a</p><p id=\"u\">b</p><p id=\"d\">c</p>").unwrap();
    let projection = script_document(&document, "page").unwrap();
    let writable = projection
        .nodes
        .iter()
        .map(|node| node.text_writable)
        .collect::<Vec<_>>();
    assert_eq!(writable, [true, true, false]);
}

/** The batch labels equal `label` per node for `for=`, wrapping, aria, text and placeholder cases. */
#[test]
fn labels_equal_label_per_node() {
    let document = parse(concat!(
        "<label for=\"n\">Name</label><input id=\"n\">",
        "<label>Email <b>here</b> <input id=\"e\"></label><label for=\"e\">Also</label>",
        "<input aria-label=\"Aria\"><input placeholder=\"Hint\">",
        "<button>Go <b>now</b></button><p>Plain <i>words</i></p>"
    ))
    .unwrap();
    let expected = document
        .nodes
        .iter()
        .map(|node| label(&document, node))
        .collect::<Vec<_>>();
    assert_eq!(document.labels(), expected);
    assert!(expected.contains(&"Email here Also".to_string()));
}

/** Wrapping labels resolve stable IDs even when neither identity nor ancestry follows vector order. */
#[test]
fn batch_labels_preserve_associations_after_identity_reordering() {
    let mut document = parse(concat!(
        "<label for=i>Explicit</label><section><label>Wrapped <b>name</b><input id=i></label></section>",
        "<p aria-label=Authored>text</p><input placeholder=Hint>"
    )).unwrap();
    for node in &mut document.nodes {
        node.local_id += NODES;
        node.parent = node.parent.map(|parent| parent + NODES);
    }
    document.nodes.reverse();
    let expected: Vec<_> = document
        .nodes
        .iter()
        .map(|node| label(&document, node))
        .collect();
    assert_eq!(document.labels(), expected);
    let mut labeler = document.labeler();
    for (index, expected) in expected.iter().enumerate() {
        assert_eq!(&labeler.label(index), expected);
    }
}

/** Labeling every node of a NODES-node page finishes well inside the call deadline. */
#[test]
fn labels_at_the_node_limit_are_linear() {
    let document = parse(&large_page()).unwrap();
    let started = Instant::now();
    let labels = document.labels();
    assert!(started.elapsed() < STAGE_BUDGET, "{:?}", started.elapsed());
    assert_eq!(labels.len(), NODES);
    assert_eq!(labels[1], "Field 0");
}

/** A wrapping label holding all static text gives each descendant a name cut to TEXT_BYTES. */
#[test]
fn labels_stay_within_text_bytes_per_node() {
    let spans = STATIC_TEXT_TOTAL_BYTES / STATIC_TEXT_NODE_BYTES;
    let span = format!(
        "<span>{}</span>",
        "\u{e9}".repeat(STATIC_TEXT_NODE_BYTES / 2)
    );
    let html = format!(
        "<label>{}{}</label>",
        span.repeat(spans),
        "<b></b>".repeat(NODES - spans - 1)
    );
    let document = parse(&html).unwrap();
    assert_eq!(document.nodes.len(), NODES);
    let started = Instant::now();
    let labels = document.labels();
    assert!(started.elapsed() < STAGE_BUDGET, "{:?}", started.elapsed());
    assert!(labels.iter().all(|label| label.len() <= TEXT_BYTES));
    // The cut lands on a character boundary, so a multibyte name may stop up to 3 bytes short.
    assert!(labels.last().unwrap().len() > TEXT_BYTES - 4);
    assert!(labels.iter().map(String::len).sum::<usize>() <= NODES * TEXT_BYTES);
}

/** A table body of `rows` one-cell rows. */
fn table(rows: usize) -> String {
    format!("<table>{}</table>", "<tr><td>x</td></tr>".repeat(rows))
}

/** A table at TABLE_ROWS is annotated with headers; one more row still parses, keeps every
cell, and only goes without header associations. */
#[test]
fn a_table_past_the_annotation_bound_still_parses() {
    let small = parse(&table(TABLE_ROWS)).unwrap();
    assert!(small.nodes.iter().any(|node| node.table_cell.is_some()));
    let large = parse(&table(TABLE_ROWS + 1)).unwrap();
    let cells = large.nodes.iter().filter(|node| node.tag == "td").count();
    assert_eq!(cells, TABLE_ROWS + 1);
    assert!(large.nodes.iter().all(|node| node.table_cell.is_none()));
    assert!(large.diagnostics.iter().any(|d| d.contains("large table")));
}

/** Static text totaling `bytes`, split into nodes at the per-node cap. */
fn text(bytes: usize) -> String {
    let mut html = String::new();
    let mut left = bytes;
    while left > 0 {
        let size = left.min(STATIC_TEXT_NODE_BYTES);
        html.push_str(&format!("<p>{}</p>", "a".repeat(size)));
        left -= size;
    }
    html
}

/** STATIC_TEXT_TOTAL_BYTES of text parses and one more byte is refused with the text limit. */
#[test]
fn static_text_limit_holds_at_the_boundary() {
    parse(&text(STATIC_TEXT_TOTAL_BYTES)).unwrap();
    let error = parse(&text(STATIC_TEXT_TOTAL_BYTES + 1)).unwrap_err();
    assert!(
        error.message.contains("document text bytes"),
        "{}",
        error.message
    );
}

/** CSS_RULES rules apply; past the bound the extra rules are dropped with a diagnostic, never the page. */
#[test]
fn css_rules_limit_holds_at_the_boundary() {
    let document = parse("<p>a</p>").unwrap();
    let at = compute_styles(&document, &["p{color:red}".repeat(CSS_RULES)]).unwrap();
    assert!(!at.diagnostics.iter().any(|d| d.contains("CSS rules")));
    let over = compute_styles(&document, &["p{color:red}".repeat(CSS_RULES + 1)]).unwrap();
    assert_eq!(over.nodes[0].style.color.as_deref(), Some("red"));
    assert!(
        over.diagnostics
            .iter()
            .any(|d| d.contains("CSS rules") && d.contains(&CSS_RULES.to_string()))
    );
}
