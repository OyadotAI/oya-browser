/** Tables outside the static subset are repaired and laid out best effort instead of refused. */
use mars_eye::{Document, Node, compute_styles, parse, render};

/** The first node with this author id. */
fn by_id<'a>(document: &'a Document, id: &str) -> &'a Node {
    document
        .nodes
        .iter()
        .find(|node| node.html_id == id)
        .unwrap_or_else(|| panic!("no #{id}"))
}

/** Row and cell end tags are implied by the next row or cell. */
#[test]
fn rows_and_cells_imply_their_end_tags() {
    let document =
        parse("<table id=t><tr id=r1><td id=a>a<td id=b>b<tr id=r2><td id=c>c</table>").unwrap();
    let r1 = by_id(&document, "r1").local_id;
    assert_eq!(by_id(&document, "a").parent, Some(r1));
    assert_eq!(by_id(&document, "b").parent, Some(r1));
    assert_eq!(
        by_id(&document, "r2").parent,
        Some(by_id(&document, "t").local_id)
    );
    let cell = by_id(&document, "c").table_cell.as_ref().unwrap();
    assert_eq!((cell.row_index, cell.column_index), (2, 1));
}

/** Row groups close one another and the rows inside them. */
#[test]
fn row_groups_imply_their_end_tags() {
    let document = parse(
        "<table id=t><thead id=h><tr><th>H<tbody id=b><tr><td id=c>x<tfoot id=f><tr><td>f</table>",
    )
    .unwrap();
    let table = by_id(&document, "t").local_id;
    for id in ["h", "b", "f"] {
        assert_eq!(by_id(&document, id).parent, Some(table), "{id}");
    }
    assert_eq!(
        by_id(&document, "c")
            .table_cell
            .as_ref()
            .unwrap()
            .headers
            .len(),
        1
    );
}

/** Cells directly in a table or row group get an implied row. */
#[test]
fn cells_outside_rows_get_an_implied_row() {
    let document = parse("<table id=t><td id=a>a</td><td id=b>b</td></table>").unwrap();
    let row = &document.nodes[by_id(&document, "a").parent.unwrap()];
    assert_eq!(row.tag, "tr");
    assert_eq!(by_id(&document, "b").parent, Some(row.local_id));
    assert!(
        document
            .diagnostics
            .iter()
            .any(|d| d.contains("implied row"))
    );
}

/** Text and elements misplaced in table structure are foster parented beside the table. */
#[test]
fn misplaced_table_content_is_foster_parented() {
    let document = parse(
        "<div id=o><table id=t>stray<div id=d>box</div><tr><td id=c>x</td></tr></table></div>",
    )
    .unwrap();
    let outer = by_id(&document, "o").local_id;
    assert_eq!(by_id(&document, "d").parent, Some(outer));
    assert_eq!(
        document
            .nodes
            .iter()
            .find(|n| n.text == "stray")
            .unwrap()
            .parent,
        Some(outer)
    );
    assert!(by_id(&document, "c").table_cell.is_some());
}

/** A form inside table structure keeps its rows, so its controls stay owned and submit. */
#[test]
fn form_inside_a_table_keeps_its_controls() {
    let document = parse(
        "<table><form id=f action=/s><tr><td><input id=q name=q value=v></td></tr></form></table>",
    )
    .unwrap();
    let form = by_id(&document, "f").local_id;
    assert_eq!(
        mars_eye::form_owner(&document, by_id(&document, "q")),
        Some(form)
    );
    assert_eq!(
        mars_eye::submission(&document, form, None).unwrap().encoded,
        "q=v"
    );
}

/** Wikipedia-shaped spans and nested infobox tables parse and record diagnostics. */
#[test]
fn wikipedia_spans_and_nested_tables_parse() {
    let document = parse(include_str!(
        "../../../fixtures/compat/wikipedia-table.html"
    ))
    .unwrap();
    assert!(document.diagnostics.iter().any(|d| d.contains("spans")));
    assert!(
        document
            .diagnostics
            .iter()
            .any(|d| d.contains("Nested tables"))
    );
    let inner = document
        .nodes
        .iter()
        .find(|n| n.text == "Inner cell")
        .unwrap();
    let cell = inner.table_cell.as_ref().unwrap();
    assert_eq!(
        document.nodes[cell.table].attributes["class"],
        "nowraplinks"
    );
    let safari = document.nodes.iter().find(|n| n.text == "Safari").unwrap();
    assert_eq!(safari.table_cell.as_ref().unwrap().row_index, 4);
}

/** Hacker News table soup, nested layout tables and all, parses with its links intact. */
#[test]
fn hacker_news_table_soup_parses() {
    let document = parse(include_str!("../../../fixtures/compat/hn-table-soup.html")).unwrap();
    assert_eq!(document.title, "Hacker News");
    let story = document
        .nodes
        .iter()
        .find(|n| n.text == "A story about browsers")
        .unwrap();
    assert_eq!(story.href.as_deref(), Some("https://example.com/story"));
    let vote = by_id(&document, "up_41000001");
    assert_eq!(
        vote.href.as_deref(),
        Some("vote?id=41000001&how=up&goto=news")
    );
    assert!(
        document
            .nodes
            .iter()
            .any(|n| n.text == "Ask HN: Table soup & you")
    );
    assert!(
        document
            .nodes
            .iter()
            .any(|n| n.tag == "input" && n.attributes["name"] == "q")
    );
}

/** A spanning, nested table renders instead of refusing. */
#[test]
fn spanning_nested_table_renders() {
    let document = parse(
        "<table><tr><th colspan=2>Wide</th></tr><tr><td rowspan=3>Tall</td><td><table><tr><td>Inner</td></tr></table></td></tr></table>",
    )
    .unwrap();
    let display = render(&compute_styles(&document, &[]).unwrap(), 640.0).unwrap();
    for text in ["Wide", "Tall", "Inner"] {
        assert!(
            display
                .items
                .iter()
                .any(|item| item.lines.iter().any(|line| line.text == text)),
            "{text}"
        );
    }
}

/** A table opened directly inside another table closes the first, as HTML does. */
#[test]
fn table_in_table_context_closes_the_first() {
    let document =
        parse("<table id=a><tr><td>x</td></tr><table id=b><tr><td>y</td></tr></table>").unwrap();
    assert_eq!(by_id(&document, "b").parent, None);
}

/** Outside any table, cell, row and row-group start tags are parse errors and ignored; text stays. */
#[test]
fn stray_table_parts_outside_a_table_are_ignored() {
    let document = parse("<div id=d><tr><td id=c>cell</td><th>head</th></tr></div>").unwrap();
    let tags: Vec<_> = document
        .nodes
        .iter()
        .map(|node| node.tag.as_str())
        .collect();
    assert!(
        !tags.iter().any(|tag| ["tr", "td", "th"].contains(tag)),
        "{tags:?}"
    );
    let text: String = document
        .nodes
        .iter()
        .map(|node| node.text.as_str())
        .collect();
    assert_eq!(text, "cellhead");
    assert!(document.nodes.iter().all(|node| node.table_cell.is_none()));
    assert_eq!(
        document.diagnostics,
        ["A table part outside any table was ignored; its text stays as text."]
    );
}
