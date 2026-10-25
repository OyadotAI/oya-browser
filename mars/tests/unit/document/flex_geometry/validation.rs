/** Unsupported and forged private layout inputs fail before native geometry is published. */
use super::*;

/** Reversed directions, reversed wrapping, percentage gaps and baseline alignment are approximated
 * (source order, start alignment, the percentage part dropped) without a diagnostic. */
#[test]
fn approximated_flex_values_style_without_a_diagnostic() {
    for css in [
        "display:flex;flex-direction:row-reverse",
        "display:flex;flex-wrap:wrap-reverse",
        "display:flex;gap:10%",
        "display:flex;align-items:baseline",
    ] {
        let source = format!("<div style='{css}'><p>a</p><p>b</p></div>");
        let document = compute_styles(&parse(&source).unwrap(), &[]).unwrap();
        assert!(document.diagnostics.is_empty(), "{css}");
        assert_eq!(render(&document, 500.0).unwrap().items.len(), 3, "{css}");
    }
}

/** `inline-flex` is a supported atomic inline flex container, so it styles without a diagnostic. */
#[test]
fn inline_flex_is_an_atomic_inline_flex_container() {
    let source = "<div style='display:inline-flex'><p>a</p><p>b</p></div>";
    let document = compute_styles(&parse(source).unwrap(), &[]).unwrap();
    assert!(document.diagnostics.is_empty());
    let style = &document.nodes[0].style;
    assert!(style.display_flex && style.inline_block);
    let display = render(&document, 500.0).unwrap();
    assert_eq!(display.items.len(), 3);
    assert_eq!(display.items[1].y, display.items[2].y);
}

/** Unsupported flex modes and values are dropped or approximated with a named diagnostic. */
#[test]
fn unsupported_layout_declarations_degrade_with_a_diagnostic() {
    for (css, diagnostic) in [
        (
            "display:flex;flex-direction:column;gap:10",
            "A CSS value for gap is not supported and was ignored.",
        ),
        (
            "display:flex;flex-basis:fit-content(10px)",
            "A CSS value for flex-basis is not supported and was ignored.",
        ),
    ] {
        let source = format!("<div style='{css}'><p>a</p><p>b</p></div>");
        let document = compute_styles(&parse(&source).unwrap(), &[]).unwrap();
        assert_eq!(document.diagnostics, [diagnostic], "{css}");
        assert!(document.nodes[0].style.display_flex, "{css}");
        let display = render(&document, 500.0).unwrap();
        assert_eq!(display.items.len(), 3, "{css}");
    }
}

/** A wrapped column stacks its items in one column, as nowrap would, without overlap. */
#[test]
fn wrapped_columns_lay_out_as_one_nowrap_column() {
    let source =
        "<div style='display:flex;flex-direction:column;flex-wrap:wrap'><p>a</p><p>b</p></div>";
    let display = render(
        &compute_styles(&parse(source).unwrap(), &[]).unwrap(),
        500.0,
    )
    .unwrap();
    let (a, b) = (&display.items[1], &display.items[2]);
    assert_eq!(a.x, b.x);
    assert!(b.y >= a.y + a.height);
}

/** The UA stylesheet's 2px table border-spacing. */
const TABLE_SPACING: f64 = 2.0;

/** Flex over tables or table structure falls back to block flow, keeping every box, with a diagnostic. */
#[test]
fn unsupported_flex_tree_shapes_fall_back_to_block_flow() {
    for (body, boxes, inset) in [
        (
            "<div style='display:flex'><table><tr><td>cell</td></tr></table></div>",
            4,
            0.0,
        ),
        (
            "<table style='display:flex'><tr><td>cell</td></tr></table>",
            3,
            TABLE_SPACING,
        ),
    ] {
        let document = compute_styles(&parse(body).unwrap(), &[]).unwrap();
        assert_eq!(
            document.diagnostics,
            ["A flex container over text, tables or table structure was laid out as block flow."],
            "{body}"
        );
        let display = render(&document, 500.0).unwrap();
        assert_eq!(display.items.len(), boxes, "{body}");
        // Block flow keeps the first child at the container's left edge; a table's own
        // border-spacing insets its rows, as Blink's table layout does.
        let first = &display.items[0];
        assert_eq!(display.items[1].x, first.x + inset, "{body}");
        assert!(
            display.items[1..]
                .iter()
                .all(|child| child.x >= first.x && child.y >= first.y),
            "{body}"
        );
    }
}

/** Bare text runs (leading or trailing) and content `<button>` elements participate in flex layout without diagnostics. */
#[test]
fn flex_containers_wrap_bare_text_and_content_buttons_as_flex_items() {
    for body in [
        "<div style='display:flex;gap:8px'>before<div>child</div></div>",
        "<div style='display:flex;gap:8px'><div>child</div>after</div>",
        "<button style='display:flex;gap:8px'><span>Icon</span>Save</button>",
    ] {
        let document = compute_styles(&parse(body).unwrap(), &[]).unwrap();
        assert!(document.diagnostics.is_empty(), "{body}");
        let display = render(&document, 500.0).unwrap();
        assert!(display.items[1].x >= display.items[0].x, "{body}");
    }
    let lead = render(
        &compute_styles(
            &parse("<div style='display:flex;gap:8px'>before<div>child</div></div>").unwrap(),
            &[],
        )
        .unwrap(),
        500.0,
    )
    .unwrap();
    assert!(!lead.items[0].lines.is_empty());
    assert!(lead.items[1].x > lead.items[0].x);
}

/** Replaced images, percentage radii/insets/transforms, typography decorations and SVG stroke/fill rules resolve into the display list. */
#[test]
fn day_to_day_visual_upgrades_resolve_into_display_items() {
    let html = concat!(
        "<div style='position:relative;width:200px;height:100px'>",
        "<img src='a.png' width='80' height='80' style='object-fit:cover;object-position:25% 75%;border-radius:50%;padding:4px'>",
        "<div style='position:absolute;top:50%;left:50%;transform:translate(-50%, -50%);width:80px;height:20px'><em><u><s>Badge</s></u></em></div>",
        "<svg width='24' height='24' viewBox='0 0 24 24'><path d='M2 12 L22 12' style='stroke:#112233;stroke-width:2px;stroke-linecap:round;fill-rule:evenodd;fill-opacity:50%;stroke-opacity:0.75'/></svg>",
        "</div>"
    );
    let document = compute_styles(&parse(html).unwrap(), &[]).unwrap();
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
    let display = render(&document, 500.0).unwrap();
    let img = display.items.iter().find(|item| item.tag == "img").unwrap();
    assert_eq!(img.object_fit, mars::document::ObjectFit::Cover);
    assert_eq!(img.object_position, [0.0, 25.0, 0.0, 75.0]);
    assert!(img.radius[0] >= 40.0);
    assert!(img.content_box.is_some());

    let badge = display
        .items
        .iter()
        .find(|item| item.lines.iter().any(|l| l.text.contains("Badge")))
        .unwrap();
    assert!((badge.x - (display.items[0].x + 100.0 - 40.0)).abs() < 1e-6);
    assert!((badge.y - (display.items[0].y + 50.0 - 10.0)).abs() < 1e-6);
    assert!(badge.italic);
    assert!(badge.underline);
    assert!(badge.strikethrough);

    let svg = display.items.iter().find(|item| item.tag == "svg").unwrap();
    assert_eq!(svg.vector.len(), 1);
    let path = &svg.vector[0];
    assert_eq!(path.line_cap, mars::document::LineCap::Round);
    assert_eq!(path.fill_rule, mars::document::FillRule::EvenOdd);
    assert!((path.fill_opacity - 0.5).abs() < 1e-6);
    assert!((path.stroke_opacity - 0.75).abs() < 1e-6);
}

/** Private wire data cannot bypass CSS numeric ranges and poison native geometry. */
#[test]
fn forged_computed_dimensions_are_rejected_at_the_render_boundary() {
    for (field, value) in [("width", -1.0), ("font_size", 5000.0), ("border", 5000.0)] {
        let mut wire = serde_json::to_value(parse("<div></div>").unwrap()).unwrap();
        wire["nodes"][0]["style"] = serde_json::json!({field:value});
        let document: Document = serde_json::from_value(wire).unwrap();
        assert_eq!(
            render(&document, 500.0).unwrap_err().code,
            "invalid_request",
            "{field}"
        );
    }
}

/** Renderer traversal validates parent topology and unique IDs before following any edges. */
#[test]
fn malformed_private_trees_are_rejected_instead_of_looping_or_dropping_nodes() {
    let document = parse("<div><p>child</p></div>").unwrap();
    let mut cycle = document.clone();
    cycle.nodes[0].parent = Some(cycle.nodes[1].local_id);
    let mut duplicate = document.clone();
    duplicate.nodes[1].local_id = duplicate.nodes[0].local_id;
    let mut orphan = document;
    orphan.nodes[1].parent = Some(9999);
    for forged in [cycle, duplicate, orphan] {
        assert_eq!(render(&forged, 500.0).unwrap_err().code, "invalid_request");
    }
}

/** A private request cannot evade the parser's independent node-work allowance. */
#[test]
fn private_document_node_overflow_is_bounded_before_layout() {
    let mut document = parse("<div></div>").unwrap();
    let template = document.nodes[0].clone();
    document.nodes = (0..=mars::constants::NODES)
        .map(|id| {
            let mut node = template.clone();
            node.local_id = id;
            node
        })
        .collect();
    assert_eq!(render(&document, 500.0).unwrap_err().code, "limit_exceeded");
}
