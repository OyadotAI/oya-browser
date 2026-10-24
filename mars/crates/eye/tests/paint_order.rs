/** Paint order follows Blink's stacking contexts, z-order lists and normal-flow paint phases
 * (CSS 2.1 Appendix E): each test is one rule, with markup after Blink's and WPT's own cases. */
use mars_eye::{DisplayList, Document, compute_styles, parse, render};

/** Styles and lays out `markup` with an optional stylesheet. */
fn layout(markup: &str, css: &str) -> (Document, DisplayList) {
    let sheets = if css.is_empty() {
        Vec::new()
    } else {
        vec![css.to_owned()]
    };
    let document = compute_styles(&parse(markup).unwrap(), &sheets).unwrap();
    let display = render(&document, 800.0).unwrap();
    (document, display)
}
/** The paint index of the first item of element `id`. */
fn at(document: &Document, display: &DisplayList, id: &str) -> usize {
    let local = document
        .nodes
        .iter()
        .find(|node| node.html_id == id)
        .unwrap_or_else(|| panic!("no #{id}"))
        .local_id;
    display
        .items
        .iter()
        .position(|item| item.local_id == local)
        .unwrap_or_else(|| panic!("#{id} painted nothing"))
}
/** Asserts the elements paint in exactly the listed order. */
fn order(markup: &str, css: &str, ids: &[&str]) {
    let (document, display) = layout(markup, css);
    let indexes: Vec<usize> = ids.iter().map(|id| at(&document, &display, id)).collect();
    assert!(
        indexes.windows(2).all(|pair| pair[0] < pair[1]),
        "expected paint order {ids:?}, got indexes {indexes:?}"
    );
}

/** A positioned `z-index: auto` box is stacked but is no stacking context: its z-index 2 child
 * joins the root's list and paints above a z-index 1 box that comes later in the tree. */
#[test]
fn z_auto_positioned_box_does_not_contain_its_children() {
    order(
        "<div id=outer style='position:relative'>O<div id=high style='position:absolute;z-index:2'>H</div></div>\
         <div id=low style='position:relative;z-index:1'>L</div>",
        "",
        &["outer", "low", "high"],
    );
}

/** A box with z-index set is a stacking context: its children stay inside it, so a z-index 9
 * child of a z-index 1 box paints below a later z-index 2 sibling. */
#[test]
fn stacking_context_contains_its_children() {
    order(
        "<div id=one style='position:relative;z-index:1'>A<div id=nine style='position:absolute;z-index:9'>N</div></div>\
         <div id=two style='position:relative;z-index:2'>B</div>",
        "",
        &["one", "nine", "two"],
    );
}

/** A negative z-index child of a z-auto positioned box joins the root's negative list and paints
 * below that box and below normal flow. */
#[test]
fn negative_child_of_z_auto_box_paints_below_its_parent() {
    order(
        "<div id=flow>F</div><div id=parent style='position:relative'>P\
         <div id=under style='position:absolute;z-index:-1'>U</div></div>",
        "",
        &["under", "flow", "parent"],
    );
}

/** Floats paint after the backgrounds of in-flow blocks, even ones that come later in the tree. */
#[test]
fn floats_paint_above_later_block_backgrounds() {
    order(
        "<div id=float style='float:left;width:50px;height:50px;background:green'></div>\
         <div id=block style='background:red;height:20px'></div>",
        "",
        &["block", "float"],
    );
}

/** A float paints atomically: its own block children paint with it, not with the page's blocks. */
#[test]
fn float_paints_its_subtree_atomically() {
    order(
        "<div id=float style='float:left;width:50px'><div id=inner style='background:green;height:9px'></div></div>\
         <div id=block style='background:red;height:20px'></div>",
        "",
        &["block", "float", "inner"],
    );
}

/** `isolation: isolate`, `filter` and `mix-blend-mode` make stacking contexts, so a z-index 5
 * child inside stays below a later positioned z-index 1 sibling. */
#[test]
fn grouping_properties_create_stacking_contexts() {
    for property in [
        "isolation:isolate",
        "filter:blur(1px)",
        "mix-blend-mode:multiply",
        "will-change:transform",
        "contain:paint",
    ] {
        order(
            &format!(
                "<div id=group style='position:relative;{property}'>G<div id=five style='position:absolute;z-index:5'>5</div></div>\
                 <div id=one style='position:relative;z-index:1'>1</div>"
            ),
            "",
            &["group", "five", "one"],
        );
    }
}

/** z-index applies to flex items without positioning: the z-index 1 item paints over its later
 * sibling, and a positive z flex item paints above in-flow content. */
#[test]
fn z_index_applies_to_flex_items() {
    order(
        "<div style='display:flex'><div id=top style='z-index:1;margin-right:-20px'>T</div><div id=next>N</div></div>",
        "",
        &["next", "top"],
    );
}

/** A positioned `::before` with `z-index: -1` paints beneath its element's in-flow children
 * when the element is a stacking context. */
#[test]
fn negative_generated_box_paints_under_its_element_content() {
    let (document, display) = layout(
        "<div id=card class=c><p id=text>T</p></div>",
        ".c{position:relative;z-index:0}.c::before{content:'';position:absolute;inset:0;z-index:-1;background:red}",
    );
    let local = document
        .nodes
        .iter()
        .find(|node| node.html_id == "card")
        .unwrap()
        .local_id;
    let before = display
        .items
        .iter()
        .position(|item| item.local_id == local && item.tag == "::before")
        .expect("::before painted");
    assert!(at(&document, &display, "card") < before);
    assert!(before < at(&document, &display, "text"));
}
