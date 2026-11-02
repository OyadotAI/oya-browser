/** Rows, wrapped lines and nested columns preserve one coherent native geometry. */
use super::*;

/** Wrapping reserves separate row and column gaps and uses each line's tallest item. */
#[test]
fn wrapping_places_the_next_line_below_the_tallest_item() {
    let document = styled(
        "<div id=row><div id=a></div><div id=b></div><div id=c></div></div>",
        "#row{display:flex;flex-wrap:wrap;width:250px;gap:12px 10px;align-items:flex-start}#a,#b,#c{flex:0 0 100px;padding:10px 0}#b{padding:20px 0}",
    );
    let display = render(&document, 500.0).unwrap();
    let a = item(&document, &display, "a");
    let b = item(&document, &display, "b");
    let c = item(&document, &display, "c");
    close(b.x - a.x, 110.0);
    close(c.x, a.x);
    close(c.y - a.y, 52.0);
}

/** Centering and cross-axis end alignment place the boxes, rather than only their text. */
#[test]
fn main_and_cross_axis_alignment_move_the_actual_boxes() {
    let document = styled(
        "<div id=row><div id=a></div><div id=b></div></div>",
        "#row{display:flex;width:300px;gap:20px;justify-content:center;align-items:flex-end}#a,#b{flex:0 0 60px;padding:10px 0}#b{padding:40px 0}",
    );
    let display = render(&document, 500.0).unwrap();
    let row = item(&document, &display, "row");
    let a = item(&document, &display, "a");
    close(a.x - row.x, 80.0);
    close(a.y - row.y, 60.0);
    close(item(&document, &display, "b").x - a.x, 80.0);
}

/** Space-between adds remaining free space to the existing gap without changing item widths. */
#[test]
fn space_between_uses_remaining_space_after_the_authored_gap() {
    let document = styled(
        "<div id=row><div id=a></div><div id=b></div><div id=c></div></div>",
        "#row{display:flex;width:300px;gap:10px;justify-content:space-between}#a,#b,#c{flex:0 0 50px;padding:10px 0}",
    );
    let display = render(&document, 500.0).unwrap();
    let a = item(&document, &display, "a");
    let b = item(&document, &display, "b");
    let c = item(&document, &display, "c");
    close(a.width, 50.0);
    close(b.x - a.x, 125.0);
    close(c.x + c.width - a.x, 300.0);
}

/** An auto-height column derives its height from descendants and its row gap. */
#[test]
fn nested_column_height_contains_all_children_without_block_gaps() {
    let document = styled(
        "<div id=row><div id=column><div id=a></div><div id=b></div></div><div id=c></div></div>",
        "#row{display:flex;width:300px;align-items:flex-start;gap:10px}#column{display:flex;flex-direction:column;flex:0 0 100px;gap:7px}#a{padding:10px 0}#b{padding:15px 0}#c{flex:0 0 100px;padding:5px 0}",
    );
    let display = render(&document, 500.0).unwrap();
    let column = item(&document, &display, "column");
    close(column.height, 57.0);
    close(
        item(&document, &display, "b").y - item(&document, &display, "a").y,
        27.0,
    );
    close(item(&document, &display, "row").height, 57.0);
}

/** Hidden descendants consume neither a flex slot nor a gap and never become paint targets. */
#[test]
fn hidden_item_removes_its_subtree_and_inter_item_space() {
    let document = styled(
        "<div id=row><div id=a></div><div id=hidden><button id=secret>Hidden</button></div><div id=b></div></div>",
        "#row{display:flex;width:300px;gap:10px}#a,#b,#hidden{flex:0 0 50px;padding:10px 0}#hidden{display:none}",
    );
    let display = render(&document, 500.0).unwrap();
    let hidden = document
        .nodes
        .iter()
        .find(|node| node.html_id == "hidden")
        .unwrap()
        .local_id;
    let secret = document
        .nodes
        .iter()
        .find(|node| node.html_id == "secret")
        .unwrap()
        .local_id;
    assert!(
        !display
            .items
            .iter()
            .any(|item| [hidden, secret].contains(&item.local_id))
    );
    close(
        item(&document, &display, "b").x - item(&document, &display, "a").x,
        60.0,
    );
}

/** Default stretch enlarges an auto cross-size item to the largest sibling's outer height. */
#[test]
fn stretch_uses_the_measured_line_cross_extent() {
    let document = styled(
        "<div id=row><div id=a></div><div id=b></div></div>",
        "#row{display:flex;width:300px}#a,#b{flex:0 0 100px;padding:10px 0}#b{padding:30px 0}",
    );
    let display = render(&document, 500.0).unwrap();
    close(item(&document, &display, "a").height, 60.0);
    close(item(&document, &display, "b").height, 60.0);
}

/** Column bases remain distinct while the automatic vertical minimum prevents content overlap. */
#[test]
fn indefinite_column_preserves_bases_and_auto_minimum_content_sizes() {
    let body = "<div id=column><div id=a>First content</div><div id=b>Second content</div></div>";
    for (basis, expected) in [
        ("1", mars::document::FlexBasis::Percent(0.0)),
        ("1 1 0px", mars::document::FlexBasis::Pixels(0.0)),
    ] {
        let document = styled(
            body,
            &format!(
                "#column{{display:flex;flex-direction:column;width:200px;gap:8px}}#a,#b{{flex:{basis}}}"
            ),
        );
        let display = render(&document, 500.0).unwrap();
        let a = item(&document, &display, "a");
        let b = item(&document, &display, "b");
        assert_eq!(
            document
                .nodes
                .iter()
                .find(|node| node.html_id == "a")
                .unwrap()
                .style
                .flex_basis,
            expected
        );
        assert!(a.height > 0.0 && b.height > 0.0);
        close(b.y - a.y - a.height, 8.0);
        let column = item(&document, &display, "column");
        assert!(column.y + column.height >= b.y + b.height);
    }
}

/** An actual stretched row supplies definite column height that its growing children can share. */
#[test]
fn stretched_column_distributes_real_definite_height_without_overlap() {
    let document = styled(
        "<div id=row><div id=column><div id=a>First</div><div id=b>Second</div></div><div id=tall></div></div>",
        "#row{display:flex;width:400px}#column{display:flex;flex-direction:column;flex:1;min-width:0}#a,#b{flex:1 1 0px}#tall{flex:0 0 100px;padding:60px 0}",
    );
    let display = render(&document, 600.0).unwrap();
    let column = item(&document, &display, "column");
    let a = item(&document, &display, "a");
    let b = item(&document, &display, "b");
    close(column.height, 120.0);
    close(a.height, 60.0);
    close(b.height, 60.0);
    close(b.y, a.y + a.height);
    close(column.y + column.height, b.y + b.height);
}
