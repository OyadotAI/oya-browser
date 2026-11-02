/** Table box tree and column elements after Blink's layout/table: anonymous rows around loose
cells (CSS 2.1 17.2.1), header and footer group order, `rowspan=0`, `<col>` constraints
(ColumnConstraintsBuilder) and row-group borders in the collapsed model. */
use mars_eye::{Document, compute_styles, parse, render};

/** Lays `markup` out at 800px with no body margin; returns the document and its items. */
fn layout(markup: &str) -> (Document, mars_eye::DisplayList) {
    let markup = format!("<body style='margin:0;font:16px sans-serif'>{markup}");
    let document = compute_styles(&parse(&markup).unwrap(), &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    (document, display)
}

/** (x, y, width, height) of `#id`. */
fn place(markup: &str, id: &str) -> (f64, f64, f64, f64) {
    let (document, display) = layout(markup);
    let local = document
        .nodes
        .iter()
        .find(|n| n.html_id == id)
        .unwrap()
        .local_id;
    let item = display.items.iter().find(|i| i.local_id == local);
    let item = item.unwrap_or_else(|| panic!("#{id} not laid out"));
    (item.x, item.y, item.width, item.height)
}

/** Within half a pixel. */
fn near(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() < 0.5
}

/** Cells directly in a CSS table before a row group share one anonymous row above it. */
#[test]
fn loose_cells_form_an_anonymous_row() {
    let cell = "display:table-cell;width:50px;height:20px";
    let markup = format!(
        "<div style='display:table;border-spacing:0'>\
         <div id=a style='{cell}'></div><div id=b style='{cell}'></div>\
         <div style='display:table-row-group'><div style='display:table-row'>\
         <div id=c style='{cell}'></div><div style='{cell}'></div></div></div></div>"
    );
    let (a, b, c) = (
        place(&markup, "a"),
        place(&markup, "b"),
        place(&markup, "c"),
    );
    assert!(near(a.1, 0.0) && near(b.1, 0.0), "{a:?} {b:?}");
    assert!(near(b.0, 50.0), "{b:?}");
    assert!(near(c.1, 20.0) && near(c.0, 0.0), "{c:?}");
}

/** The first header group lays out first and the first footer group last, whatever the source
 * order. */
#[test]
fn header_first_footer_last() {
    let markup = "<table style='border-spacing:0'>\
                  <tfoot><tr><td id=f style='height:10px;padding:0'></td></tr></tfoot>\
                  <tbody><tr><td id=b style='height:20px;padding:0'></td></tr></tbody>\
                  <thead><tr><td id=h style='height:30px;padding:0'></td></tr></thead></table>";
    let (h, b, f) = (place(markup, "h"), place(markup, "b"), place(markup, "f"));
    assert!(near(h.1, 0.0), "{h:?}");
    assert!(near(b.1, 30.0), "{b:?}");
    assert!(near(f.1, 50.0), "{f:?}");
}

/** `rowspan=0` spans to the end of its row group and no further. */
#[test]
fn zero_rowspan_reaches_the_section_end() {
    let td = "style='height:20px;padding:0;width:20px'";
    let markup = format!(
        "<table style='border-spacing:0'><tbody>\
         <tr><td id=s rowspan=0></td><td {td}></td></tr>\
         <tr><td {td}></td></tr><tr><td {td}></td></tr></tbody>\
         <tbody><tr><td id=n {td}></td></tr></tbody></table>"
    );
    let (s, n) = (place(&markup, "s"), place(&markup, "n"));
    assert!(near(s.3, 60.0), "{s:?}");
    assert!(near(n.1, 60.0) && near(n.0, 0.0), "{n:?}");
}

/** In a fixed table a `<col>` width sets its column over the first row's cells; the other
 * columns share the rest evenly. */
#[test]
fn fixed_table_takes_col_widths() {
    let markup = "<table style='table-layout:fixed;width:420px;border-spacing:4px'>\
                  <col><col><col style='width:40%'><col>\
                  <tr><td></td><td></td><td id=c style='width:300px'></td><td></td></tr></table>";
    let c = place(markup, "c");
    assert!(near(c.2, 160.0), "{c:?}");
    assert!(near(c.0, 172.0), "{c:?}");
}

/** In an auto table a column length constrains its column like a cell's authored width, and a
 * column group's width falls to its columns that have none. */
#[test]
fn auto_table_column_and_group_widths() {
    let cell = "display:table-cell";
    let markup = format!(
        "<div style='display:table;border-spacing:0'>\
         <div style='display:table-column-group;width:70px'><div style='display:table-column'>\
         </div><div style='display:table-column;width:30px'></div></div>\
         <div style='display:table-row'><div id=a style='{cell}'></div>\
         <div id=b style='{cell}'></div></div></div>"
    );
    assert!(
        near(place(&markup, "a").2, 70.0),
        "{:?}",
        place(&markup, "a")
    );
    assert!(
        near(place(&markup, "b").2, 30.0),
        "{:?}",
        place(&markup, "b")
    );
}

/** An empty `<colgroup>` sizes the `span` columns it covers. */
#[test]
fn colgroup_span_sizes_its_columns() {
    let markup = "<table style='border-spacing:0'><colgroup span=2 style='width:40px'></colgroup>\
                  <tr><td id=a style='padding:0'></td><td id=b style='padding:0'></td></tr></table>";
    assert!(near(place(markup, "b").0, 40.0), "{:?}", place(markup, "b"));
    assert!(near(place(markup, "b").2, 40.0), "{:?}", place(markup, "b"));
}

/** A row group's border joins the collapsed-border conflict: its wider top edge gives the table
 * and the first row's cells half of it each. */
#[test]
fn collapsed_row_group_border() {
    let markup = "<table style='border-collapse:collapse'>\
                  <tbody style='border-top:6px solid black'>\
                  <tr><td id=a style='padding:0;height:10px;width:10px'></td></tr></tbody></table>";
    let a = place(markup, "a");
    assert!(near(a.1, 3.0), "{a:?}");
    assert!(near(a.3, 13.0), "{a:?}");
}
