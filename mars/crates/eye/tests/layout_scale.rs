/** Layout and paint stay linear and inside the private RPC deadline at the page-size limits. */
use mars_eye::{
    DisplayList, Document, DomChange, InsertPosition, agent_hidden, apply_changes, compute_styles,
    constants::{
        NODES, PAINT_LINES, RPC_TIMEOUT, STATIC_TEXT_NODE_BYTES, STATIC_TEXT_TOTAL_BYTES,
        TABLE_ROWS,
    },
    parse,
    protocol::EngineResult,
    render,
};
use std::time::{Duration, Instant};

/// A render must leave room for parse, styles and the wire inside one private call deadline.
const RENDER_BUDGET: Duration = RPC_TIMEOUT.checked_div(3).unwrap();
/// The scoreboard's default window width.
const VIEWPORT: f64 = 1100.0;

/** Styled document for markup, panicking only on test-authored input. */
fn styled(markup: &str) -> Document {
    compute_styles(&parse(markup).unwrap(), &[]).unwrap()
}

/** Render with its wall time, so the deadline is asserted on layout alone. */
fn timed(document: &Document) -> (EngineResult<DisplayList>, Duration) {
    let started = Instant::now();
    let display = render(document, VIEWPORT);
    (display, started.elapsed())
}

/** Total paint lines the limit counts. */
fn lines(display: &DisplayList) -> usize {
    display.items.iter().map(|item| item.lines.len()).sum()
}

/** A flat page of NODES paragraphs paints every one inside the deadline. Short lines keep the
 * page inside LAYOUT_EXTENT, past which boxes are dropped rather than the page refused. */
#[test]
fn flat_page_at_node_limit_renders_inside_deadline() {
    let document = styled(&"<p style=\"margin:0;line-height:2px\">x</p>".repeat(NODES));
    assert_eq!(document.nodes.len(), NODES);
    let (display, elapsed) = timed(&document);
    assert_eq!(lines(&display.unwrap()), NODES);
    assert!(elapsed < RENDER_BUDGET, "{elapsed:?}");
}

/** A nested navigation-shaped page at NODES renders inside the deadline. */
#[test]
fn nested_page_at_node_limit_renders_inside_deadline() {
    let item = "<div><ul><li><a href=\"/a\">x</a></li></ul></div>";
    let document = styled(&item.repeat(NODES / 4));
    assert_eq!(document.nodes.len(), NODES);
    let (display, elapsed) = timed(&document);
    // Each list item paints its link text and its bullet.
    assert_eq!(lines(&display.unwrap()), NODES / 4 * 2);
    assert!(elapsed < RENDER_BUDGET, "{elapsed:?}");
}

/** A TABLE_ROWS by four table lays out every row and cell inside the deadline. */
#[test]
fn table_at_row_limit_renders_inside_deadline() {
    let row = "<tr><td>alpha</td><td>beta</td><td><a href=\"/c\">gamma</a></td><td>delta</td></tr>";
    let document = styled(&format!("<table>{}</table>", row.repeat(TABLE_ROWS)));
    let (display, elapsed) = timed(&document);
    let display = display.unwrap();
    let count = |tag: &str| display.items.iter().filter(|item| item.tag == tag).count();
    assert_eq!(count("tr"), TABLE_ROWS);
    assert_eq!(count("td"), TABLE_ROWS * 4);
    assert!(elapsed < RENDER_BUDGET, "{elapsed:?}");
}

/** Distinct CJK words in varied fonts defeat the measure cache; layout stays inside the deadline. */
#[test]
fn cache_defeating_text_renders_inside_deadline_or_is_refused() {
    let mut glyph = 0x4e00_u32;
    let mut html = String::new();
    let mut paragraph = 0;
    while html.len() + STATIC_TEXT_NODE_BYTES < STATIC_TEXT_TOTAL_BYTES {
        let mut words = String::new();
        while words.len() + 4 < STATIC_TEXT_NODE_BYTES {
            words.push(char::from_u32(glyph).unwrap());
            words.push(' ');
            glyph = 0x4e00 + (glyph - 0x4e00 + 1) % 0x5000;
        }
        let weight = if paragraph % 2 == 0 { "bold" } else { "normal" };
        let size = 10 + paragraph % 32;
        html.push_str(&format!(
            "<p style=\"font-size:{size}px;font-weight:{weight}\">{words}</p>"
        ));
        paragraph += 1;
    }
    let document = styled(&html);
    let (display, elapsed) = timed(&document);
    assert!(elapsed < RENDER_BUDGET, "{elapsed:?}");
    if let Err(error) = display {
        assert!(
            error.message.starts_with("layout measurements"),
            "{}",
            error.message
        );
    }
}

/** Exactly PAINT_LINES lines paint; one more line is dropped and the display list says it is truncated. */
#[test]
fn paint_lines_at_limit_render_and_one_more_is_truncated() {
    let four = "<p style=\"width:1px;margin:0;line-height:2px\">a b c d</p>";
    let at_limit = render(&styled(&four.repeat(NODES)), VIEWPORT).unwrap();
    assert_eq!(lines(&at_limit), PAINT_LINES);
    assert!(!at_limit.truncated);
    let over = styled(
        &(four.repeat(NODES - 1) + "<p style=\"width:1px;margin:0;line-height:2px\">a b c d e</p>"),
    );
    let display = render(&over, VIEWPORT).unwrap();
    assert_eq!(lines(&display), PAINT_LINES);
    assert!(display.truncated);
}

/** After a Move, identities no longer match positions and hidden ancestry still resolves. */
#[test]
fn agent_hidden_follows_parents_after_a_move() {
    let document = parse(
        "<p>x</p><div aria-hidden=\"true\"><span>y</span></div>\
         <table><tr><td>z</td></tr></table>",
    )
    .unwrap();
    let cell = document.nodes.iter().find(|node| node.tag == "td").unwrap();
    let moved = apply_changes(
        &document,
        &[DomChange::Move {
            node: 0,
            target: Some(cell.local_id),
            position: InsertPosition::BeforeEnd,
        }],
    )
    .unwrap();
    let moved = compute_styles(&moved, &[]).unwrap();
    assert_ne!(moved.nodes[0].local_id, 0);
    let node = |id: usize| moved.nodes.iter().find(|node| node.local_id == id).unwrap();
    assert!(agent_hidden(&moved, node(2)));
    assert!(!agent_hidden(&moved, node(0)));
    let display = render(&moved, VIEWPORT).unwrap();
    let cell = display.items.iter().find(|item| item.tag == "td").unwrap();
    let paragraph = display
        .items
        .iter()
        .find(|item| item.local_id == 0)
        .unwrap();
    assert!(paragraph.y >= cell.y && paragraph.x >= cell.x);
}

/** A table longer than the old 1024-row grid lays out every row, as a long Hacker News thread
needs: none is dropped. */
#[test]
fn a_long_table_lays_out_every_row() {
    const ROWS: usize = 1500;
    let markup = format!(
        "<table>{}</table>",
        "<tr><td style='height:10px'>x</td></tr>".repeat(ROWS)
    );
    let document = styled(&markup);
    let display = mars_eye::render(&document, 800.0).unwrap();
    let cells = display
        .items
        .iter()
        .filter(|item| item.lines.iter().any(|l| l.text == "x"))
        .count();
    assert_eq!(cells, ROWS);
}
