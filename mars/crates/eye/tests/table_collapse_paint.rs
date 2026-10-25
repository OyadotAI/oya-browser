/** Collapsed table borders paint as Blink's CollapsedBorderPainter does: each grid edge at its
whole width in the winning border's color, and row groups paint a background box under their
rows. */
use mars_eye::{DisplayItem, compute_styles, parse, render};

/** Every display item `#id` produced, laying out `markup` at 800px with no body margin. */
fn items(markup: &str, id: &str) -> Vec<DisplayItem> {
    let markup = format!("<body style='margin:0'>{markup}");
    let document = compute_styles(&parse(&markup).unwrap(), &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    let local = document
        .nodes
        .iter()
        .find(|n| n.html_id == id)
        .unwrap()
        .local_id;
    display
        .items
        .into_iter()
        .filter(|i| i.local_id == local)
        .collect()
}

/** A 10px collapsed cell border paints 10px wide, centered on the grid line, not the 5px half
 * the cell holds for layout. */
#[test]
fn a_collapsed_edge_paints_at_its_whole_width() {
    let cell = items(
        "<table style='border-collapse:collapse'><tr><td id=c style='border:10px solid red;\
         width:50px;height:50px;padding:0'></td></tr></table>",
        "c",
    );
    let edges = cell.last().unwrap();
    assert_eq!(edges.border_widths, [10.0; 4], "{:?}", edges.border_widths);
    assert!(cell[0].border_widths.iter().all(|w| *w == 0.0));
    assert_eq!(edges.width - cell[0].width, 10.0);
}

/** On a tie in width the stronger style wins the edge and paints in its own color. */
#[test]
fn the_stronger_style_wins_a_tied_edge() {
    let cell = items(
        "<table style='border-collapse:collapse'><tr><td id=a style='border:4px dotted blue'>a</td>\
         <td id=b style='border:4px solid green'>b</td></tr></table>",
        "a",
    );
    let edges = cell.last().unwrap();
    assert_eq!(edges.border_colors[1].as_deref(), Some("green"));
    assert_eq!(edges.border_colors[3].as_deref(), Some("blue"));
}

/** A row group with a background paints one box under its rows. */
#[test]
fn a_row_group_paints_its_background() {
    let group = items(
        "<table><tbody id=g style='background:lime'><tr><td>a</td></tr><tr><td>b</td></tr>\
         </tbody></table>",
        "g",
    );
    assert_eq!(group.len(), 1);
    assert_eq!(group[0].background.as_deref(), Some("#00ff00"));
    assert!(group[0].height > 30.0, "{}", group[0].height);
}

/** A transformed ancestor is the containing block of a fixed descendant (Blink's
 * CanContainFixedPositionObjects), so `top: 0; left: 0` lands on its padding box. */
#[test]
fn a_transform_contains_fixed_descendants() {
    let fixed = items(
        "<div style='height:50px'></div><div style='transform:translateX(0);margin-left:30px'>\
         <div id=f style='position:fixed;top:0;left:0;width:10px;height:10px'></div></div>",
        "f",
    );
    assert_eq!(
        (fixed[0].x, fixed[0].y),
        (30.0, 50.0),
        "{:?}",
        (fixed[0].x, fixed[0].y)
    );
}
