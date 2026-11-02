/** Flow-relative margins, paddings, insets, borders and sizes map to physical sides by direction. */
use mars_eye::{Document, Node, compute_styles, parse};

/** Styles `markup` with one sheet. */
fn styled(markup: &str, sheet: &str) -> Document {
    compute_styles(&parse(markup).unwrap(), &[sheet.into()]).unwrap()
}
/** The node with `id`. */
fn by_id<'a>(document: &'a Document, id: &str) -> &'a Node {
    document.nodes.iter().find(|n| n.html_id == id).unwrap()
}

#[test]
fn inline_start_is_left_in_ltr_and_right_in_rtl() {
    let sheet = "div{margin-inline-start:10px;padding-inline-end:4px}";
    let document = styled("<div id=a>a</div><div id=b dir=rtl>b</div>", sheet);
    let (a, b) = (&by_id(&document, "a").style, &by_id(&document, "b").style);
    assert_eq!((a.margin[3], a.margin[1], a.padding[1]), (10.0, 0.0, 4.0));
    assert_eq!((b.margin[1], b.margin[3], b.padding[3]), (10.0, 0.0, 4.0));
}

#[test]
fn css_direction_flips_the_mapping_and_inherits() {
    let document = styled(
        "<section><div id=a>a</div></section>",
        "section{direction:rtl} div{margin-inline-start:8px}",
    );
    let a = &by_id(&document, "a").style;
    assert!(a.rtl);
    assert_eq!((a.margin[1], a.margin[3]), (8.0, 0.0));
}

#[test]
fn block_shorthands_take_one_or_two_values() {
    let document = styled(
        "<div id=a>a</div>",
        "div{margin-block:1px 2px;padding-inline:3px;padding-block:5px}",
    );
    let a = &by_id(&document, "a").style;
    assert_eq!((a.margin[0], a.margin[2]), (1.0, 2.0));
    assert_eq!(a.padding, [5.0, 3.0, 5.0, 3.0]);
}

#[test]
fn logical_and_physical_spellings_compete_by_cascade_order() {
    let later_logical = styled(
        "<div id=a>a</div>",
        "div{margin-left:1px;margin-inline-start:2px}",
    );
    assert_eq!(by_id(&later_logical, "a").style.margin[3], 2.0);
    let later_physical = styled(
        "<div id=a>a</div>",
        "div{margin-inline-start:2px;margin-left:1px}",
    );
    assert_eq!(by_id(&later_physical, "a").style.margin[3], 1.0);
}

#[test]
fn insets_borders_and_sizes_map_too() {
    let document = styled(
        "<div id=a>a</div>",
        "div{position:relative;inset-inline-start:3px;inset-block-start:4px;\
         border-inline-start:2px solid red;inline-size:50px;block-size:20px}",
    );
    let a = &by_id(&document, "a").style;
    assert_eq!((a.inset[3], a.inset[0]), (Some(3.0), Some(4.0)));
    assert_eq!(a.width, Some(50.0));
    assert_eq!(a.height.map(|h| h.px), Some(20.0));
    assert_eq!(a.border_widths[3], 2.0);
    assert!(
        document.diagnostics.iter().all(|d| !d.contains("inline")),
        "{:?}",
        document.diagnostics
    );
}

#[test]
fn logical_values_resolve_through_custom_properties() {
    let document = styled(
        "<div id=a>a</div>",
        ":root{--g:6px 7px} div{margin-inline:var(--g)}",
    );
    let a = &by_id(&document, "a").style;
    assert_eq!((a.margin[3], a.margin[1]), (6.0, 7.0));
}
