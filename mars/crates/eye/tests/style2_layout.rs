/** Self alignment, `order`, percentage paddings and margins, and `aspect-ratio` in layout. */
use mars_eye::{DisplayItem, DisplayList, Document, compute_styles, parse, render};

/** Styles and lays out `markup` with one sheet at an 800-point viewport. */
fn laid(markup: &str, sheet: &str) -> (Document, DisplayList) {
    let document = compute_styles(&parse(markup).unwrap(), &[sheet.into()]).unwrap();
    let display = render(&document, 800.0).unwrap();
    (document, display)
}
/** The first box painted for the element with `id`. */
fn item<'a>(document: &Document, display: &'a DisplayList, id: &str) -> &'a DisplayItem {
    let node = document.nodes.iter().find(|n| n.html_id == id).unwrap();
    display
        .items
        .iter()
        .find(|item| item.local_id == node.local_id)
        .unwrap()
}

#[test]
fn align_self_overrides_align_items_in_a_flex_row() {
    let (document, display) = laid(
        "<div id=row><div id=a>a</div><div id=b>b</div></div>",
        "#row{display:flex;align-items:flex-start;height:100px}#a,#b{height:20px}\
         #b{align-self:center}",
    );
    let row = item(&document, &display, "row");
    assert_eq!(item(&document, &display, "a").y, row.y);
    assert_eq!(item(&document, &display, "b").y, row.y + 40.0);
}

#[test]
fn align_self_stretch_and_end_apply_per_item() {
    let (document, display) = laid(
        "<div id=row><div id=a>a</div><div id=b>b</div></div>",
        "#row{display:flex;align-items:center;height:100px}#b{height:10px;align-self:flex-end}\
         #a{align-self:stretch}",
    );
    let row = item(&document, &display, "row");
    assert_eq!(item(&document, &display, "a").height, 100.0);
    assert_eq!(item(&document, &display, "b").y, row.y + 90.0);
}

#[test]
fn align_self_positions_items_across_a_flex_column() {
    let (document, display) = laid(
        "<div id=col><div id=a>a</div></div>",
        "#col{display:flex;flex-direction:column;width:200px}#a{width:50px;align-self:center}",
    );
    let col = item(&document, &display, "col");
    assert_eq!(item(&document, &display, "a").x, col.x + 75.0);
}

#[test]
fn order_rearranges_flex_items_and_keeps_source_order_among_equals() {
    let (document, display) = laid(
        "<div id=row><div id=a>a</div><div id=b>b</div><div id=c>c</div></div>",
        "#row{display:flex}#row>div{width:10px}#a{order:2}#b{order:1}#c{order:1}",
    );
    let x = |id| item(&document, &display, id).x;
    assert!(x("b") < x("c") && x("c") < x("a"));
}

#[test]
fn justify_self_and_align_self_place_grid_items() {
    let (document, display) = laid(
        "<div id=g><div id=a>a</div></div>",
        "#g{display:grid;grid-template-columns:200px;grid-template-rows:100px}\
         #a{width:50px;height:20px;justify-self:end;align-self:center}",
    );
    let g = item(&document, &display, "g");
    let a = item(&document, &display, "a");
    assert_eq!((a.x, a.y), (g.x + 150.0, g.y + 40.0));
}

#[test]
fn percentage_padding_top_resolves_against_the_containing_width() {
    // YouTube's thumbnail box: zero content and `padding-top: 56.25%` of a 400px wide parent.
    let (document, display) = laid(
        "<div id=p><div id=thumb></div></div>",
        "#p{width:400px}#thumb{padding-top:56.25%}",
    );
    assert_eq!(item(&document, &display, "thumb").height, 225.0);
}

#[test]
fn percentage_margins_and_horizontal_paddings_use_the_width_on_every_side() {
    let (document, display) = laid(
        "<div id=p><div id=c>x</div></div>",
        "#p{width:400px;height:50px;overflow:hidden}#c{margin:10% 0 0 25%;padding:0 5%;height:10px}",
    );
    let p = item(&document, &display, "p");
    let c = item(&document, &display, "c");
    assert_eq!(c.x, p.x + 100.0);
    assert_eq!(c.y, p.y + 40.0);
    assert_eq!(c.width, 300.0);
}

#[test]
fn calc_mixing_pixels_and_percent_padding_keeps_both_parts() {
    let (document, display) = laid(
        "<div id=p><div id=c></div></div>",
        "#p{width:200px}#c{padding-bottom:calc(50% + 4px)}",
    );
    assert_eq!(item(&document, &display, "c").height, 104.0);
}

#[test]
fn aspect_ratio_gives_an_automatic_height() {
    let (document, display) = laid(
        "<div id=a></div><div id=b></div><div id=c></div>",
        "div{width:320px}#a{aspect-ratio:16/9}#b{aspect-ratio:2;padding:10px;box-sizing:border-box}\
         #c{aspect-ratio:1;height:30px}",
    );
    assert_eq!(item(&document, &display, "a").height, 180.0);
    assert_eq!(item(&document, &display, "b").height, 160.0);
    assert_eq!(item(&document, &display, "c").height, 30.0);
}

#[test]
fn hostile_percentages_and_ratios_stay_bounded() {
    let (_, display) = laid(
        "<div id=a>a</div><div id=b>b</div>",
        "#a{padding-top:1e9%;margin-left:-1e9%}#b{aspect-ratio:1/0}#b{aspect-ratio:0.0000001}",
    );
    assert!(
        display
            .items
            .iter()
            .all(|i| i.height.is_finite() && i.y.is_finite())
    );
}

#[test]
fn env_values_use_their_fallback_or_zero() {
    let (document, display) = laid(
        "<div id=p><div id=c>x</div></div>",
        "#p{width:400px;overflow:hidden}\
         #c{padding-left:max(22px, env(safe-area-inset-left));padding-right:env(x, 7px);\
         margin-top:calc(4px + env(safe-area-inset-top))}",
    );
    let (p, c) = (
        item(&document, &display, "p"),
        item(&document, &display, "c"),
    );
    assert_eq!(c.x, p.x);
    assert_eq!(c.y, p.y + 4.0);
    let c = document.nodes.iter().find(|n| n.html_id == "c").unwrap();
    assert_eq!((c.style.padding[3], c.style.padding[1]), (22.0, 7.0));
}
