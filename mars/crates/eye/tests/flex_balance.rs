/** `flex-wrap: balance` and `flex-line-count`, ported from Blink's `BalanceBreakFlexItemsIntoLines`. */
use mars_eye::{DisplayItem, DisplayList, Document, compute_styles, parse, render};

/** A styled document and its display list at an 800 point viewport. */
fn layout(markup: &str) -> (Document, DisplayList) {
    let sheet = "body{margin:0}".to_owned();
    let document = compute_styles(&parse(markup).unwrap(), &[sheet]).unwrap();
    let display = render(&document, 800.0).unwrap();
    (document, display)
}

/** The (x, y) of the first item painted for each element with an id in `ids`. */
fn spots(markup: &str, ids: &[&str]) -> Vec<(f64, f64)> {
    let (document, display) = layout(markup);
    ids.iter()
        .map(|id| {
            let local = document
                .nodes
                .iter()
                .find(|node| node.html_id == *id)
                .unwrap()
                .local_id;
            let item: &DisplayItem = display
                .items
                .iter()
                .find(|item| item.local_id == local)
                .unwrap();
            (item.x, item.y)
        })
        .collect()
}

/** Four 25px items with 10px gaps in 100px wrap 3+1 greedily; balanced they make 2+2. */
#[test]
fn balance_evens_out_line_free_space() {
    let markup = |wrap: &str| {
        format!(
            "<div style='display:flex;flex-wrap:{wrap};gap:10px;width:100px'>\
             <i id=a style='width:25px;height:25px'></i><i id=b style='width:25px;height:25px'></i>\
             <i id=c style='width:25px;height:25px'></i><i id=d style='width:25px;height:25px'></i></div>"
        )
    };
    assert_eq!(spots(&markup("wrap"), &["c"]), vec![(70.0, 0.0)]);
    assert_eq!(
        spots(&markup("balance"), &["c", "d"]),
        vec![(0.0, 35.0), (35.0, 35.0)]
    );
}

/** `flex-line-count: 4` forces four lines out of four items that would all fit on one, by
bisecting the break size and then pushing perfectly fitting items down. */
#[test]
fn min_line_count_forces_lines() {
    let markup = "<div style='display:flex;width:200px;flex-wrap:balance;flex-line-count:4'>\
         <i id=a style='width:100px;height:25px'></i><i id=b style='width:100px;height:25px'></i>\
         <i id=c style='width:100px;height:25px'></i><i id=d style='width:100px;height:25px'></i></div>";
    let at = spots(markup, &["a", "b", "c", "d"]);
    assert_eq!(at, vec![(0.0, 0.0), (0.0, 25.0), (0.0, 50.0), (0.0, 75.0)]);
}

/** Of equal-scoring breaks, the earlier lines take the most items: five items of 25px with
10px gaps at three per line split 3+2, not 2+3. */
#[test]
fn ties_favour_earlier_lines() {
    let item = |id: &str| format!("<i id={id} style='width:25px;height:10px'></i>");
    let markup = format!(
        "<div style='display:flex;flex-wrap:balance;flex-line-count:2;gap:10px;width:95px'>{}</div>",
        ["a", "b", "c", "d", "e"].map(item).concat()
    );
    assert_eq!(spots(&markup, &["c", "d"]), vec![(70.0, 0.0), (0.0, 20.0)]);
}

/** A balanced column with an indefinite height still makes `flex-line-count` lines, and each
item fits into its line's share of the width. */
#[test]
fn balanced_column_splits_without_definite_height() {
    let markup = "<div style='display:flex;flex-direction:column;flex-wrap:balance;\
         flex-line-count:2;width:200px'>\
         <i id=a style='height:50px;width:10px'></i><i id=b style='height:50px;width:10px'></i></div>";
    assert_eq!(spots(markup, &["a", "b"]), vec![(0.0, 0.0), (100.0, 0.0)]);
}

/** A legacy `-webkit-box` never wraps, so `balance` leaves its items on one line. */
#[test]
fn webkit_box_ignores_balance() {
    let markup = "<div style='display:-webkit-box;flex-wrap:balance;width:100px'>\
         <i id=a style='width:50px;height:10px'></i><i id=b style='width:50px;height:10px'></i>\
         <i id=c style='width:50px;height:10px'></i></div>";
    assert!(spots(markup, &["c"])[0].1 == 0.0);
}

/** `flex-wrap` grammar: `balance` pairs with `wrap` or `wrap-reverse` once, never `nowrap`. */
#[test]
fn flex_wrap_balance_grammar() {
    let line_of_c = |wrap: &str| {
        let markup = format!(
            "<div style='display:flex;flex-wrap:{wrap};gap:10px;width:100px'>\
             <i id=a style='width:25px;height:25px'></i><i id=b style='width:25px;height:25px'></i>\
             <i id=c style='width:25px;height:25px'></i><i id=d style='width:25px;height:25px'></i></div>"
        );
        spots(&markup, &["c"])[0]
    };
    assert_eq!(line_of_c("wrap balance"), (0.0, 35.0));
    assert_eq!(line_of_c("balance wrap"), (0.0, 35.0));
    // Invalid values are dropped, leaving the initial `nowrap` single line.
    assert_eq!(line_of_c("balance nowrap").1, 0.0);
    assert_eq!(line_of_c("balance balance").1, 0.0);
}

/** The WPT `webkit-box-no-balance` shape: three growing 50px items share one 100px line. */
#[test]
fn webkit_box_items_share_one_line() {
    let item = |id: &str| {
        format!(
            "<div id={id} style='width:50px;height:100px;-webkit-box-flex:1;flex-grow:1'></div>"
        )
    };
    let markup = format!(
        "<div style='display:-webkit-box;flex-wrap:balance;width:100px'>{}</div>",
        ["a", "b", "c"].map(item).concat()
    );
    let (document, display) = layout(&markup);
    let local = |id: &str| {
        document
            .nodes
            .iter()
            .find(|n| n.html_id == id)
            .unwrap()
            .local_id
    };
    let boxes = ["a", "b", "c"].map(|id| {
        let item = display
            .items
            .iter()
            .find(|item| item.local_id == local(id))
            .unwrap();
        (item.x, item.y, item.width, item.height)
    });
    assert_eq!(boxes.map(|b| b.1 + b.3), [100.0; 3], "{boxes:?}");
    assert!((boxes[2].0 + boxes[2].2 - 100.0).abs() < 0.01, "{boxes:?}");
}

/** The same box styled from a stylesheet, as the WPT page does. */
#[test]
fn webkit_box_from_sheet() {
    let sheet = "body{margin:0}#w{display:-webkit-box;flex-wrap:balance;width:100px}\
         #w>div{width:50px;height:100px;background:green;-webkit-box-flex:1;flex-grow:1}"
        .to_owned();
    let markup = "<div id=w><div id=a></div><div id=b></div><div id=c></div></div>";
    let document = compute_styles(&parse(markup).unwrap(), &[sheet]).unwrap();
    let display = render(&document, 800.0).unwrap();
    let local = |id: &str| {
        document
            .nodes
            .iter()
            .find(|n| n.html_id == id)
            .unwrap()
            .local_id
    };
    let boxes = ["a", "b", "c"].map(|id| {
        let item = display
            .items
            .iter()
            .find(|item| item.local_id == local(id))
            .unwrap();
        (item.x, item.y, item.width, item.height)
    });
    assert_eq!(boxes.map(|b| b.1), [0.0; 3], "{boxes:?}");
}
