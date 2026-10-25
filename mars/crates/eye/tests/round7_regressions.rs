/** Round-5 and round-6 regressions: flex in table cells and character-reference carriage returns. */
use mars_eye::{Document, compute_styles, parse, render};

/** A styled document at an 800 point viewport with no body margin. */
fn styled(markup: &str) -> Document {
    compute_styles(&parse(markup).unwrap(), &["body{margin:0}".to_owned()]).unwrap()
}

/** A flex container inside a table cell stays flex: its wrapped items sit in rows, as Blink
lays out flex inside table cells (css-flexbox/flexbox_rowspan). */
#[test]
fn flex_inside_a_table_cell_stays_flex() {
    let document = styled(
        "<table><tr><td><div id=f style='display:flex;flex-wrap:wrap;width:300px'>\
         <i id=a style='display:block;width:100px;height:10px'></i>\
         <i id=b style='display:block;width:100px;height:10px'></i></div></td></tr></table>",
    );
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
    let display = render(&document, 800.0).unwrap();
    let spot = |id: &str| {
        let local = document
            .nodes
            .iter()
            .find(|n| n.html_id == id)
            .unwrap()
            .local_id;
        let item = display.items.iter().find(|i| i.local_id == local).unwrap();
        (item.x, item.y)
    };
    let (a, b) = (spot("a"), spot("b"));
    assert_eq!(a.1, b.1, "flex items share one line");
    assert_eq!(b.0, a.0 + 100.0);
}

/** `&#x0D;` produces a real U+000D in the text; only literal CRs in the source become LF. */
#[test]
fn character_reference_carriage_return_survives_newline_normalization() {
    let document = parse("<div>A&#x0D;B\r\nC\rD</div>").unwrap();
    let text: String = document
        .nodes
        .iter()
        .map(|node| node.text.clone())
        .collect();
    assert_eq!(text, "A\rB\nC\nD");
}
