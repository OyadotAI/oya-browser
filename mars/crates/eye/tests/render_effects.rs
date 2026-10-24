/** Positioned layout, paint order, clips, decorations, transforms and inline controls in the display list. */
use mars_eye::{DisplayItem, DisplayList, Document, compute_styles, parse, render};

/** A styled document and its display list at a fixed viewport. */
fn layout(markup: &str) -> (Document, DisplayList) {
    let document = compute_styles(&parse(markup).unwrap(), &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    (document, display)
}
/** The item and its paint index for an element id. */
fn find<'a>(document: &Document, display: &'a DisplayList, id: &str) -> (usize, &'a DisplayItem) {
    let local = document
        .nodes
        .iter()
        .find(|node| node.html_id == id)
        .unwrap()
        .local_id;
    display
        .items
        .iter()
        .enumerate()
        .find(|(_, item)| item.local_id == local)
        .unwrap()
}

/** Absolute boxes sit at their insets inside the positioned ancestor and leave flow untouched. */
#[test]
fn absolute_box_uses_insets_and_leaves_flow() {
    let with = "<div id=c style='position:relative;border:2px solid #000'><p id=a>One</p>\
        <div id=abs style='position:absolute;top:10px;left:20px;width:50px'>X</div><p id=b>Two</p></div>";
    let without = "<div id=c style='position:relative;border:2px solid #000'><p id=a>One</p><p id=b>Two</p></div>";
    let (document, display) = layout(with);
    let (_, container) = find(&document, &display, "c");
    let (_, abs) = find(&document, &display, "abs");
    assert_eq!(abs.x, container.x + 2.0 + 20.0);
    assert_eq!(abs.y, container.y + 2.0 + 10.0);
    assert_eq!(abs.width, 50.0);
    let (plain_document, plain) = layout(without);
    assert_eq!(
        find(&document, &display, "b").1.y,
        find(&plain_document, &plain, "b").1.y
    );
    assert_eq!(
        container.height,
        find(&plain_document, &plain, "c").1.height
    );
}

/** Right and bottom insets anchor against the containing block's far edges. */
#[test]
fn right_and_bottom_insets_anchor_far_edges() {
    let (document, display) = layout(
        "<div id=c style='position:relative;width:300px;padding:0'><p>a</p><p>b</p><p>c</p>\
         <div id=abs style='position:absolute;right:0;bottom:0;width:40px'>X</div></div>",
    );
    let (_, container) = find(&document, &display, "c");
    let (_, abs) = find(&document, &display, "abs");
    assert_eq!(abs.x + abs.width, container.x + container.width);
    assert!((abs.y + abs.height - (container.y + container.height)).abs() < 0.001);
}

/** Relative offsets move the box and its text without moving following siblings. */
#[test]
fn relative_offset_moves_box_and_text_only() {
    let (document, display) =
        layout("<p id=r style='position:relative;top:5px;left:7px'>Moved</p><p id=n>Next</p>");
    let (plain_document, plain) = layout("<p id=r>Moved</p><p id=n>Next</p>");
    let (_, moved) = find(&document, &display, "r");
    let (_, original) = find(&plain_document, &plain, "r");
    assert_eq!(moved.x, original.x + 7.0);
    assert_eq!(moved.y, original.y + 5.0);
    assert_eq!(moved.lines[0].x, original.lines[0].x + 7.0);
    assert_eq!(moved.lines[0].y, original.lines[0].y + 5.0);
    assert_eq!(
        find(&document, &display, "n").1.y,
        find(&plain_document, &plain, "n").1.y
    );
}

/** Higher z-index paints later regardless of source order; negative z paints below flow content. */
#[test]
fn z_index_sets_paint_order() {
    let (document, display) = layout(
        "<div style='position:relative'>\
         <div id=high style='position:absolute;z-index:2;top:0'>H</div>\
         <div id=low style='position:absolute;z-index:1;top:0'>L</div>\
         <p id=flow>F</p>\
         <div id=under style='position:absolute;z-index:-1;top:0'>U</div></div>",
    );
    let (high, high_item) = find(&document, &display, "high");
    let (low, _) = find(&document, &display, "low");
    let (flow, _) = find(&document, &display, "flow");
    let (under, _) = find(&document, &display, "under");
    assert!(under < flow && flow < low && low < high);
    assert_eq!(high_item.z, 2);
}

/** Nested overflow clips intersect; a fixed box escapes every ancestor clip. */
#[test]
fn overflow_clips_intersect_and_fixed_escapes() {
    let (document, display) = layout(
        "<div id=outer style='overflow:hidden;width:200px'>\
         <div id=inner style='overflow:hidden;width:300px;margin-left:50px'><p id=leaf>Text</p>\
         <div id=fixed style='position:fixed;top:0;left:0'>F</div></div></div>",
    );
    let (_, outer) = find(&document, &display, "outer");
    let (_, inner) = find(&document, &display, "inner");
    let (_, leaf) = find(&document, &display, "leaf");
    let clip = leaf.clip.unwrap();
    assert_eq!(clip.x, inner.x);
    assert_eq!(clip.x + clip.width, outer.x + outer.width);
    assert!(inner.clip.is_some() && outer.clip.is_none());
    assert!(find(&document, &display, "fixed").1.clip.is_none());
}

/** Radius, shadows and gradients reach the display list resolved against the box. */
#[test]
fn decorations_pass_through_resolved() {
    let (document, display) = layout(
        "<div id=d style='width:100px;border-radius:8px;box-shadow:0 2px 4px #00000080;\
         background-image:linear-gradient(to right, red, blue)'>D</div>",
    );
    let (_, item) = find(&document, &display, "d");
    assert_eq!(item.radius, [8.0; 4]);
    assert_eq!(item.shadows.len(), 1);
    assert_eq!((item.shadows[0].y, item.shadows[0].blur), (2.0, 4.0));
    let (radial, [x0, y0, x1, y1], stops) = item.backgrounds[0].gradient().unwrap();
    assert!(!radial);
    assert!(x0.abs() < 1e-9 && (x1 - item.width).abs() < 1e-9);
    assert!((y0 - item.height / 2.0).abs() < 1e-9 && (y1 - y0).abs() < 1e-9);
    assert_eq!(
        stops.iter().map(|stop| stop.position).collect::<Vec<_>>(),
        [0.0, 1.0]
    );
}

/** Translations are baked into geometry so agent bounds equal paint; other transforms are carried. */
#[test]
fn translations_bake_and_scales_carry() {
    let (document, display) = layout(
        "<p id=t style='transform:translate(10px, 5px)'>T</p><p id=s style='transform:scale(2)'>S</p>",
    );
    let (plain_document, plain) = layout("<p id=t>T</p><p id=s>S</p>");
    let (_, moved) = find(&document, &display, "t");
    let (_, original) = find(&plain_document, &plain, "t");
    assert!(moved.transform.is_none());
    assert_eq!((moved.x, moved.y), (original.x + 10.0, original.y + 5.0));
    let (_, scaled) = find(&document, &display, "s");
    let [a, _, _, d, _, _] = scaled.transform.unwrap();
    assert_eq!((a, d), (2.0, 2.0));
}

/** Translucent ancestors become opacity groups, while per-item opacity keeps the product. */
#[test]
fn translucent_ancestor_is_one_group() {
    let (document, display) =
        layout("<div id=g style='opacity:0.5'><p id=c style='opacity:0.5'>C</p></div>");
    let (_, group) = find(&document, &display, "g");
    let (_, child) = find(&document, &display, "c");
    assert_eq!(child.opacity, 0.25);
    assert_eq!(child.stacking_opacity.len(), 2);
    assert_eq!(child.stacking_opacity[0].id, group.local_id);
    assert_eq!(child.stacking_opacity[0].opacity, 0.5);
}

/** Labels, inputs and buttons share one line box as inline-level boxes. */
#[test]
fn controls_flow_inline_with_text() {
    let (document, display) = layout(
        "<div><label id=l for=n>Your name</label><input id=n><button id=b>Say hello</button></div>",
    );
    let (_, label) = find(&document, &display, "l");
    let (_, input) = find(&document, &display, "n");
    let (_, button) = find(&document, &display, "b");
    // One line box: baseline alignment may offset tops, but every box overlaps the others.
    for (a, b) in [(label, input), (input, button), (label, button)] {
        assert!(a.y < b.y + b.height && b.y < a.y + a.height);
    }
    assert!(label.x + label.width <= input.x && input.x + input.width <= button.x);
    assert!(input.width < 200.0 && button.width < 120.0);
}

/** Hostile insets, z-indexes and transforms produce bounded geometry or a typed error, never a panic. */
#[test]
fn hostile_effects_stay_bounded() {
    for style in [
        "position:absolute;top:1e300px;left:-1e300px",
        "position:relative;z-index:2147483647;top:999999999px",
        "transform:scale(1e300)",
        "position:absolute;right:0;bottom:0;border-radius:1e300px",
    ] {
        let markup = format!("<div style='{style}'>x<div style='position:absolute'>y</div></div>");
        let Ok(document) = compute_styles(&parse(&markup).unwrap(), &[]) else {
            continue;
        };
        if let Ok(display) = render(&document, 800.0) {
            assert!(
                display
                    .items
                    .iter()
                    .all(|item| item.x.is_finite() && item.y.is_finite())
            );
        }
    }
}
