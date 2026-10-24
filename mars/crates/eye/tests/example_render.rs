/** Layout and vector paint for example.com after its script: CJK wrapping, the root link and the svg icon. */
mod support;
use mars_eye::{
    Appearance, DisplayItem, LineJoin, PathCommand, agent_hidden, compute_styles, parse, render,
};
use support::{applied, display};

/** One svg's display item from standalone markup. */
fn icon(markup: &str) -> mars_eye::protocol::EngineResult<DisplayItem> {
    let document = compute_styles(&parse(markup)?, &[])?;
    let display = render(&document, 640.0)?;
    Ok(display
        .items
        .into_iter()
        .find(|item| item.tag == "svg")
        .unwrap())
}

/** Every command endpoint, relative to the item origin. */
fn points(item: &DisplayItem, path: usize) -> Vec<(f64, f64)> {
    item.vector[path]
        .commands
        .iter()
        .filter_map(|command| match *command {
            PathCommand::Move { x, y } | PathCommand::Line { x, y } => {
                Some((x - item.x, y - item.y))
            }
            PathCommand::Cubic { x, y, .. } => Some((x - item.x, y - item.y)),
            PathCommand::Close => None,
        })
        .collect()
}

/** The Chinese paragraph wraps inside the 26em column and never starts a line with closing punctuation. */
#[test]
fn chinese_paragraph_wraps_within_max_width() {
    let (document, display) = display("en", Appearance::Light);
    let zh = document
        .nodes
        .iter()
        .find(|node| node.attributes.get("lang").is_some_and(|lang| lang == "zh"))
        .unwrap();
    let item = display
        .items
        .iter()
        .find(|item| item.local_id == zh.local_id)
        .unwrap();
    assert!(item.lines.len() > 1);
    assert_eq!(item.width, 26.0 * 16.0);
    for line in &item.lines {
        assert!(line.width <= item.width);
        assert!(
            !line
                .text
                .starts_with(['，', '。', '、', '；', '：', '？', '！', '）'])
        );
    }
    let joined = item
        .lines
        .iter()
        .map(|line| line.text.as_str())
        .collect::<String>();
    assert_eq!(joined, zh.text);
}

/** The root-level link is its own centred anonymous line, and its bounds are exactly its paint. */
#[test]
fn root_level_link_bounds_equal_its_paint() {
    let (_, display) = display("en", Appearance::Light);
    let link = display.items.iter().find(|item| item.tag == "a").unwrap();
    assert_eq!(link.role, "link");
    let line = &link.lines[0];
    assert_eq!((link.x, link.width), (line.x, line.width));
    let paragraph = display.items.iter().find(|item| item.tag == "p").unwrap();
    assert!((link.x + link.width / 2.0 - (paragraph.x + paragraph.width / 2.0)).abs() < 0.01);
    let last = display.items.iter().rfind(|item| item.tag == "p").unwrap();
    assert!(link.y >= last.y + last.height);
}

/** The icon emits three paths painted in the text color in both appearances, with a scaled stroke. */
#[test]
fn icon_vector_uses_current_color_and_scaled_stroke() {
    for (appearance, color) in [
        (Appearance::Light, "#000000"),
        (Appearance::Dark, "#ffffff"),
    ] {
        let (_, display) = display("en", appearance);
        let item = display.items.iter().find(|item| item.tag == "svg").unwrap();
        assert_eq!(item.vector.len(), 3);
        assert_eq!(display.canvas_foreground.as_deref(), Some(color));
        let outline = &item.vector[0];
        assert_eq!(outline.fill, None);
        assert_eq!(outline.stroke.as_deref(), Some(color));
        assert!((outline.stroke_width - 1.3 * 2.2).abs() < 1e-9);
        assert_eq!(outline.line_join, LineJoin::Round);
        for mark in &item.vector[1..] {
            assert_eq!(mark.fill.as_deref(), Some(color));
            assert_eq!(mark.stroke, None);
        }
        assert!(item.lines.is_empty());
    }
}

/** The `use` copy is the referenced path moved 2.4 viewBox units along the rotated x axis. */
#[test]
fn rotated_use_copy_is_offset_by_its_x() {
    let (_, display) = display("en", Appearance::Light);
    let item = display.items.iter().find(|item| item.tag == "svg").unwrap();
    let (original, copy) = (points(item, 1), points(item, 2));
    let (sin, cos) = (-6.0_f64).to_radians().sin_cos();
    let scale = 44.0 / 20.0;
    for (a, b) in original.iter().zip(&copy) {
        assert!((b.0 - a.0 - 2.4 * cos * scale).abs() < 1e-9);
        assert!((b.1 - a.1 - 2.4 * sin * scale).abs() < 1e-9);
    }
}

/** Absolute, relative, implicit-repeat and compact-number path data resolve to the same points. */
#[test]
fn path_data_handles_relative_implicit_and_compact_forms() {
    let item = icon(
        "<svg viewBox='0 0 10 10' width=10 height=10>\
         <path d='M1 1L3 1 3 3H1V1z'/>\
         <path d='m1,1 2,0l0 2h-2v-2Z'/>\
         <path d='M1 1h2v2h-2z'/>\
         <path d='M.5.5l1.5-0.5.5.6'/>\
         </svg>",
    )
    .unwrap();
    let square = [(1.0, 1.0), (3.0, 1.0), (3.0, 3.0), (1.0, 3.0), (1.0, 1.0)];
    assert_eq!(points(&item, 0), square);
    assert_eq!(points(&item, 1), square);
    assert_eq!(points(&item, 2)[..4], square[..4]);
    let compact = points(&item, 3);
    assert_eq!(compact[0], (0.5, 0.5));
    assert_eq!(compact[1], (2.0, 0.0));
    assert!((compact[2].0 - 2.5).abs() < 1e-9 && (compact[2].1 - 0.6).abs() < 1e-9);
    assert_eq!(item.vector[0].fill.as_deref(), Some("black"));
}

/** Quadratic segments become the equivalent cubic. */
#[test]
fn quadratic_converts_to_cubic() {
    let item =
        icon("<svg viewBox='0 0 10 10' width=10 height=10><path d='M0 0Q3 6 6 0'/></svg>").unwrap();
    let PathCommand::Cubic {
        x1,
        y1,
        x2,
        y2,
        x,
        y,
    } = item.vector[0].commands[1]
    else {
        panic!("expected a cubic");
    };
    // Line alignment moves the whole box, so compare relative positions within rounding.
    let relative = |a: f64, b: f64| {
        let round = |v: f64| (v * 1e6).round() / 1e6;
        (round(a - item.x), round(b - item.y))
    };
    assert_eq!(relative(x1, y1), (2.0, 4.0));
    assert_eq!(relative(x2, y2), (4.0, 4.0));
    assert_eq!(relative(x, y), (6.0, 0.0));
}

/** The diagnostic for a shape render drops. */
const DROPPED: &str = "An SVG shape Eye cannot draw yet was skipped.";

/** Styled diagnostics beside the svg item, for markup whose drawing is partly unusable. */
fn drawn(markup: &str) -> (Vec<String>, DisplayItem) {
    let document = compute_styles(&parse(markup).unwrap(), &[]).unwrap();
    let item = render(&document, 640.0)
        .unwrap()
        .items
        .into_iter()
        .find(|item| item.tag == "svg")
        .unwrap();
    (document.diagnostics, item)
}

/** One path longer than SVG_PATH_SEGMENTS is dropped alone; the bound still holds. */
#[test]
fn svg_path_segment_limit_drops_only_that_path() {
    let data = format!("M0 0{}", "z".repeat(mars_eye::constants::SVG_PATH_SEGMENTS));
    let (notes, item) = drawn(&format!(
        "<svg width=10 height=10><path d='{data}'/><path d='M0 0L1 1'/></svg>"
    ));
    assert_eq!(notes, [DROPPED]);
    assert_eq!(item.vector.len(), 1);
    assert_eq!(item.vector[0].commands.len(), 2);
}

/** Unsupported transforms, bad references and malformed data drop only their own shape. */
#[test]
fn unsupported_svg_shapes_are_dropped_with_a_diagnostic() {
    for markup in [
        "<svg><g transform='perspective(2)'><path d='M0 0'/></g><path d='M1 1'/></svg>",
        "<svg><g id=a></g><use href=#a /><path d='M1 1'/></svg>",
        "<svg><use href=#missing /><path d='M1 1'/></svg>",
        "<svg width=10 height=10><path stroke=black d='L10 10 L20 20'/><path d='M1 1'/></svg>",
        "<svg width=10 height=10><path d='M0 0 A1 1 0 2 1 5 5'/><path d='M1 1'/></svg>",
        "<svg width=10 height=10><path stroke=black d='Z L5 5'/><path d='M1 1'/></svg>",
        "<svg width=10 height=10><path fill='url(#g)' d='M0 0'/><path d='M1 1'/></svg>",
    ] {
        let (notes, item) = drawn(markup);
        assert_eq!(notes, [DROPPED], "{markup}");
        assert_eq!(item.vector.len(), 1, "{markup}");
        assert_eq!(points(&item, 0), [(1.0, 1.0)], "{markup}");
    }
}

/** A malformed viewBox or size is ignored: default size, user units map 1:1. */
#[test]
fn malformed_svg_size_and_view_box_are_ignored() {
    let (notes, item) = drawn("<svg viewBox='0 0 0 10' width=x><path d='M1 2'/></svg>");
    assert_eq!(
        notes,
        ["A malformed SVG width, height or viewBox was ignored."]
    );
    assert_eq!((item.width, item.height), (300.0, 150.0));
    assert_eq!(points(&item, 0), [(1.0, 2.0)]);
}

/** C, S (reflecting the last control), T and exponent numbers resolve to the specified cubics. */
#[test]
fn cubic_smooth_and_exponent_forms_resolve() {
    let item = icon(
        "<svg viewBox='0 0 100 100' width=100 height=100>\
         <path d='M0 0C10 0 20 10 20 20S30 40 40 40'/>\
         <path d='M0 0Q10 10 20 0T40 0t20 0'/>\
         <path d='M1e1 2E1l1e+1-1e0'/>\
         </svg>",
    )
    .unwrap();
    let cubics = |path: usize| -> Vec<[f64; 6]> {
        item.vector[path]
            .commands
            .iter()
            .filter_map(|command| match *command {
                PathCommand::Cubic {
                    x1,
                    y1,
                    x2,
                    y2,
                    x,
                    y,
                } => Some([x1, y1, x2, y2, x, y].map(|v| v - item.x)),
                _ => None,
            })
            .collect()
    };
    // S mirrors (20, 10) through (20, 20).
    assert_eq!(cubics(0)[1], [20.0, 30.0, 30.0, 40.0, 40.0, 40.0]);
    // T mirrors Q's control (10, 10) through (20, 0) to (30, -10); t mirrors again to (50, 10).
    let two_thirds = |from: f64, control: f64| from + 2.0 / 3.0 * (control - from);
    assert_eq!(cubics(1)[1][1], two_thirds(0.0, -10.0));
    assert!((cubics(1)[2][1] - two_thirds(0.0, 10.0)).abs() < 1e-9);
    assert_eq!(points(&item, 2), [(10.0, 20.0), (20.0, 19.0)]);
}

/** Arcs become cubics on the true ellipse, with compact flags and radii scaled up to reach the end. */
#[test]
fn elliptical_arcs_become_cubics_on_the_ellipse() {
    let item = icon(
        "<svg viewBox='0 0 100 100' width=100 height=100>\
         <path d='M10 50A40 40 0 0 1 90 50'/>\
         <path d='M10 50a1 1 0 01 80 0'/>\
         <path d='M0 0A0 5 0 0 1 10 10'/>\
         </svg>",
    )
    .unwrap();
    for path in [0, 1] {
        let ends = points(&item, path);
        // A half circle about (50, 50) of radius 40, in two quarter-turn cubics through (50, 10).
        assert_eq!(ends.len(), 3, "{path}");
        let (x, y) = ends[1];
        assert!(
            (x - 50.0).abs() < 1e-9 && (y - 10.0).abs() < 1e-9,
            "{ends:?}"
        );
        let (x, y) = ends[2];
        assert!(
            (x - 90.0).abs() < 1e-9 && (y - 50.0).abs() < 1e-9,
            "{ends:?}"
        );
    }
    // A zero radius draws a straight segment to the end point.
    assert_eq!(points(&item, 2), [(0.0, 0.0), (10.0, 10.0)]);
}

/** The svg and everything inside it are hidden from agents; ordinary content is not. */
#[test]
fn svg_subtree_is_agent_hidden() {
    let document = applied("en");
    for node in &document.nodes {
        let inside = ["svg", "g", "path", "use"].contains(&node.tag.as_str());
        assert_eq!(agent_hidden(&document, node), inside, "{}", node.tag);
    }
    let plain = parse("<svg width=4 height=4><path d='M0 0'/></svg>").unwrap();
    assert!(!agent_hidden(&plain, &plain.nodes[0]));
    assert!(agent_hidden(&plain, &plain.nodes[1]));
}

/** Drawing children produce no display items of their own. */
#[test]
fn svg_children_produce_no_display_items() {
    let (_, display) = display("en", Appearance::Dark);
    assert!(
        display
            .items
            .iter()
            .all(|item| !["g", "path", "use"].contains(&item.tag.as_str()))
    );
    assert_eq!(
        display.items.len(),
        8,
        "the inserted style element is hidden"
    );
}
