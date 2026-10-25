/** Inline SVG is one replaced box whose bounded path subset becomes absolute vector paint. */
use super::flow::*;
use super::path::{Segment, parse};
use crate::constants::CSS_MAX_SCALAR;
use serde::{Deserialize, Serialize};

/// The SVG default replaced size when width or height is absent.
const DEFAULT_SIZE: (f64, f64) = (300.0, 150.0);

/// One painted path in document coordinates; the native drawing seam fills, then strokes.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct VectorPath {
    /// Absolute logical-point commands after viewBox, transform and `use` offsets.
    pub commands: Vec<PathCommand>,
    /// Literal fill color; None paints no fill.
    pub fill: Option<String>,
    /// Literal stroke color; None paints no stroke.
    pub stroke: Option<String>,
    /// Stroke width already scaled by the viewBox's uniform factor.
    pub stroke_width: f64,
    /// Corner style where stroked segments meet.
    pub line_join: LineJoin,
    /// End-cap style for open stroked subpaths.
    #[serde(default, skip_serializing_if = "is_butt")]
    pub line_cap: LineCap,
    /// Winding rule used when filling compound paths.
    #[serde(default, skip_serializing_if = "is_non_zero")]
    pub fill_rule: FillRule,
    /// Combined fill opacity (`opacity * fill-opacity`).
    #[serde(default = "unit_opacity", skip_serializing_if = "is_unit_opacity")]
    pub fill_opacity: f64,
    /// Combined stroke opacity (`opacity * stroke-opacity`).
    #[serde(default = "unit_opacity", skip_serializing_if = "is_unit_opacity")]
    pub stroke_opacity: f64,
}

impl VectorPath {
    /// Moves every command by a document offset, for relative positioning and translations.
    pub fn translate(&mut self, dx: f64, dy: f64) {
        for command in &mut self.commands {
            match command {
                PathCommand::Move { x, y } | PathCommand::Line { x, y } => {
                    *x += dx;
                    *y += dy;
                }
                PathCommand::Cubic {
                    x1,
                    y1,
                    x2,
                    y2,
                    x,
                    y,
                } => {
                    for (px, py) in [(x1, y1), (x2, y2), (x, y)] {
                        *px += dx;
                        *py += dy;
                    }
                }
                PathCommand::Close => {}
            }
        }
    }
}

/// Drawing commands every native backend can replay without SVG knowledge.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum PathCommand {
    /// Starts a subpath.
    Move { x: f64, y: f64 },
    /// Straight segment from the current point.
    Line { x: f64, y: f64 },
    /// Cubic Bezier from the current point; quadratics arrive converted.
    Cubic {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        x: f64,
        y: f64,
    },
    /// Closes the current subpath.
    Close,
}

/// SVG `stroke-linejoin` values the native seam supports.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LineJoin {
    #[default]
    Miter,
    Round,
    Bevel,
}

/// SVG `stroke-linecap` values the native seam supports.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LineCap {
    #[default]
    Butt,
    Round,
    Square,
}

/// SVG `fill-rule` values the native seam supports.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FillRule {
    #[default]
    NonZero,
    EvenOdd,
}

fn is_butt(cap: &LineCap) -> bool {
    *cap == LineCap::Butt
}
fn is_non_zero(rule: &FillRule) -> bool {
    *rule == FillRule::NonZero
}
fn unit_opacity() -> f64 {
    1.0
}
fn is_unit_opacity(opacity: &f64) -> bool {
    (*opacity - 1.0).abs() < 1e-9
}

/** Row-major 2D affine matrix `[a, b, c, d, e, f]` mapping (x, y) to (ax+cy+e, bx+dy+f). */
type Affine = [f64; 6];
/** The identity leaves user units untouched. */
const IDENTITY: Affine = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/** Block, inline and allocated flex svg boxes reuse replaced sizing and auto-margin centering. */
pub(super) fn svg(
    context: &mut LayoutContext<'_>,
    node: &Node,
    flow: &mut Flow,
    items: &mut Vec<DisplayItem>,
    forced: Option<BorderBox>,
) -> EngineResult<()> {
    let (x, y, width, height) = if let Some(bounds) = forced {
        let content = (bounds.width - horizontal_edges(node)).max(0.0);
        let height = bounds
            .height
            .unwrap_or(content_height(node, content, flow.definite) + vertical_edges(node));
        flow.y = bounds.y + height;
        (bounds.x, bounds.y, bounds.width, height)
    } else if node.style.inline {
        inline_box(node, flow)
    } else {
        block_box(node, flow)
    };
    flow.bottom = flow.bottom.max(flow.y);
    let start = items.len();
    let mut item = box_item(node, x, y, width, height, Vec::new());
    let current = item.color.clone().unwrap_or_else(|| "#000000".into());
    let content = (
        x + node.style.padding[3] + node.style.border,
        y + node.style.padding[0] + node.style.border,
        (width - horizontal_edges(node)).max(0.0),
        (height - vertical_edges(node)).max(0.0),
    );
    item.vector = paths(context, node, content, &current)?;
    items.push(item);
    if forced.is_none() && node.style.inline {
        let style = &node.style;
        let margin_box = height + style.margin[0] + style.margin[2];
        super::text::place_atomic(
            flow,
            (start, start + 1),
            width + style.margin[1] + style.margin[3],
            (margin_box, margin_box),
            style,
            (style.margin[3], style.margin[1]),
        );
    }
    Ok(())
}

/** Flex and table sizing see the same replaced width the svg box uses. */
pub(super) fn intrinsic_width(node: &Node) -> EngineResult<f64> {
    Ok(natural_width(node, None, None))
}
/** The content width an auto-width svg takes when its height resolves against `definite`,
 * through its viewBox ratio. */
pub(super) fn width_for_height(node: &Node, definite: Option<f64>) -> f64 {
    natural_width(node, None, definite)
}

/// The content width Chrome gives an svg: CSS width, then the width attribute, then its height
/// (CSS, then attribute) through the viewBox ratio. With only a ratio it stretches to the
/// `available` width; with nothing, or for intrinsic sizing without a ratio, it is 300 wide.
fn natural_width(node: &Node, available: Option<f64>, definite: Option<f64>) -> f64 {
    if let Some(width) = available
        .and_then(|available| super::sizing::specified_content(node, available))
        .or_else(|| dimension(node, "width"))
    {
        return width;
    }
    match (specified_height(node, definite), ratio(node)) {
        (Some(height), Some(ratio)) => height / ratio,
        (None, Some(_)) => available.unwrap_or(DEFAULT_SIZE.0),
        _ => DEFAULT_SIZE.0,
    }
}

/** The content height for a used content width: CSS height, the attribute, the viewBox ratio, then 150. */
fn content_height(node: &Node, width: f64, definite: Option<f64>) -> f64 {
    specified_height(node, definite)
        .or_else(|| ratio(node).map(|ratio| width * ratio))
        .unwrap_or(DEFAULT_SIZE.1)
}

/** A CSS height that resolves here as a content height, else the height attribute. */
fn specified_height(node: &Node, definite: Option<f64>) -> Option<f64> {
    node.style
        .height
        .and_then(|height| height.resolve_size(definite))
        .map(|height| super::sizing::content_size(node, height, false))
        .or_else(|| dimension(node, "height"))
}

/** Height over width from the viewBox, else from both size attributes. */
fn ratio(node: &Node) -> Option<f64> {
    view_box_numbers(node)
        .map(|[_, _, width, height]| height / width)
        .or_else(|| Some(dimension(node, "height")? / dimension(node, "width")?))
        .filter(|ratio| ratio.is_finite() && *ratio > 0.0)
}

/** Block svg follows ordinary block flow: collapsing margins and auto horizontal margins. */
fn block_box(node: &Node, flow: &mut Flow) -> (f64, f64, f64, f64) {
    flow.newline();
    let style = &node.style;
    let available = (flow.width - style.margin[1] - style.margin[3]).max(0.0);
    let inner = (available - horizontal_edges(node)).max(0.0);
    let natural = natural_width(node, Some(inner), flow.definite);
    let width = border_width(node, flow.width, available, Some(natural));
    let content = (width - horizontal_edges(node)).max(0.0);
    let height = content_height(node, content, flow.definite) + vertical_edges(node);
    let x = flow.x + super::body::margin_start(style, flow.width, width);
    let y = flow.block_top(style.margin[0]);
    flow.after_block(y + height, style.margin[2]);
    (x, y, width, height)
}

/** Inline svg occupies replaced geometry on the current line, like an inline image. */
fn inline_box(node: &Node, flow: &mut Flow) -> (f64, f64, f64, f64) {
    let inner = (flow.width - horizontal_edges(node)).max(0.0);
    let natural = natural_width(node, Some(inner), flow.definite);
    let width = border_width(node, flow.width, flow.width, Some(natural));
    let content = (width - horizontal_edges(node)).max(0.0);
    let height = content_height(node, content, flow.definite) + vertical_edges(node);
    let outer = width + node.style.margin[1] + node.style.margin[3];
    if flow.cursor > 0.0 && !flow.fits(outer) {
        flow.newline();
    }
    flow.fit(outer);
    let x = flow.left() + flow.cursor + node.style.margin[3];
    (x, flow.y + node.style.margin[0], width, height)
}

/** Presentation dimensions are finite nonnegative user units; a malformed one is ignored, as browsers do. */
fn dimension(node: &Node, name: &str) -> Option<f64> {
    let value = node.attributes.get(name)?.trim();
    let value = value.strip_suffix("px").unwrap_or(value);
    value
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && (0.0..=CSS_MAX_SCALAR).contains(value))
}

/// Every path and `use` copy in preorder becomes one absolute vector path. A shape Eye cannot
/// draw (bad data, transform, reference or paint) is dropped alone; `notes` reports it.
fn paths(
    context: &mut LayoutContext<'_>,
    svg: &Node,
    content: (f64, f64, f64, f64),
    current: &str,
) -> EngineResult<Vec<VectorPath>> {
    let document = context.document;
    drawing(
        &mut |work| context.charge(work),
        document,
        svg,
        content,
        current,
    )
}

/** The shapes of `svg`, a drawing inside `document`, charged to the layout's work budget. */
fn drawing(
    charge: &mut dyn FnMut(usize) -> EngineResult<()>,
    document: &Document,
    svg: &Node,
    content: (f64, f64, f64, f64),
    current: &str,
) -> EngineResult<Vec<VectorPath>> {
    let viewport = view_box(svg, content);
    let members = descendants(document, svg.local_id);
    let mut result = Vec::new();
    for node in &members {
        if definition(document, node, svg.local_id) {
            continue;
        }
        let shapes = if node.tag == "use" {
            use_copies(document, svg, node, viewport, current).unwrap_or_default()
        } else {
            shape(document, svg, node, viewport, current)
                .ok()
                .flatten()
                .into_iter()
                .collect()
        };
        for (path, segments) in shapes {
            charge(segments + 1)?;
            result.push(path);
        }
    }
    Ok(result)
}

/** Containers whose content draws only where something references it, never in place. */
const DEFINITIONS: [&str; 8] = [
    "defs",
    "symbol",
    "clippath",
    "mask",
    "pattern",
    "marker",
    "lineargradient",
    "radialgradient",
];
/** Shapes a `use` copies out of one referenced container, so a sprite cannot multiply work. */
const USE_SHAPES: usize = 256;

/** Whether a node sits inside a definition container of its drawing, which paints nothing. */
fn definition(document: &Document, node: &Node, svg: usize) -> bool {
    ancestors(document, node, svg)
        .map(|chain| chain.iter().any(|a| DEFINITIONS.contains(&a.tag.as_str())))
        .unwrap_or(true)
}

/**
 * The shapes a `use` draws: the referenced shape, or every shape of a referenced `symbol` or
 * `g`, wherever in the document it is defined (icon sprites keep their symbols in a hidden
 * svg). A symbol's viewBox maps into the use's width and height, by default the drawing's own
 * user space. Fill and stroke inherit through the referenced content, then the use.
 */
fn use_copies(
    document: &Document,
    svg: &Node,
    node: &Node,
    viewport: Affine,
    current: &str,
) -> EngineResult<Vec<(VectorPath, usize)>> {
    let id = ["href", "xlink:href"]
        .iter()
        .find_map(|name| node.attributes.get(*name))
        .and_then(|href| href.trim().strip_prefix('#'))
        .ok_or_else(unsupported)?;
    let target = document
        .nodes
        .iter()
        .find(|candidate| {
            candidate
                .attributes
                .get("id")
                .is_some_and(|value| value == id)
        })
        .ok_or_else(unsupported)?;
    let mut outer = vec![node];
    outer.extend(ancestors(document, node, svg.local_id)?);
    let mut base = use_offset(node)?;
    for element in &outer {
        base = multiply(transform(element)?, base);
    }
    if target.tag == "symbol" {
        base = multiply(base, symbol_map(target, node, svg));
    }
    let shapes: Vec<&Node> = if DRAWN.contains(&target.tag.as_str()) {
        vec![target]
    } else {
        descendants(document, target.local_id)
            .into_iter()
            .filter(|shape| DRAWN.contains(&shape.tag.as_str()))
            .take(USE_SHAPES)
            .collect()
    };
    let mut copies = Vec::with_capacity(shapes.len());
    for shape in shapes {
        let inner = if std::ptr::eq(shape, target) {
            Vec::new()
        } else {
            ancestors(document, shape, target.local_id)?
        };
        let mut matrix = transform(shape)?;
        for element in &inner {
            matrix = multiply(transform(element)?, matrix);
        }
        if !std::ptr::eq(shape, target) && target.tag != "symbol" {
            matrix = multiply(transform(target)?, matrix);
        }
        let mut paint = vec![shape];
        paint.extend(&inner);
        if !std::ptr::eq(shape, target) {
            paint.push(target);
        }
        paint.extend(&outer);
        paint.push(svg);
        let segments = parse(&path_data(shape, view_box_numbers(svg))?)?;
        let matrix = multiply(viewport, multiply(base, matrix));
        copies.push((
            vector(document, &segments, matrix, &paint, current, viewport[0])?,
            segments.len(),
        ));
    }
    Ok(copies)
}

/** A symbol's viewBox fitted into the use's width and height by the symbol's own
 * `preserveAspectRatio`; those default to the referencing drawing's viewBox size, the user space
 * a `use` fills at 100%. */
fn symbol_map(symbol: &Node, node: &Node, svg: &Node) -> Affine {
    let Some(numbers) = view_box_numbers(symbol) else {
        return IDENTITY;
    };
    let room = view_box_numbers(svg).map(|[_, _, w, h]| (w, h));
    let size = |name: &str, fallback: Option<f64>| {
        node.attributes
            .get(name)
            .and_then(|value| value.trim().trim_end_matches("px").parse::<f64>().ok())
            .filter(|value| value.is_finite() && *value > 0.0)
            .or(fallback)
    };
    let width = size("width", room.map(|r| r.0)).unwrap_or(numbers[2]);
    let height = size("height", room.map(|r| r.1)).unwrap_or(numbers[3]);
    aspect_transform(numbers, aspect_ratio(symbol), (0.0, 0.0, width, height))
}

/** Whether replaced vector content is clipped to its content box: always for an img, and for
 * inline svg unless it asks for `overflow: visible`, as Blink's UA sheet gives
 * `svg:not(:root)` hidden overflow. */
pub(super) fn clips_content(node: &Node) -> bool {
    if node.tag != "svg" {
        return true;
    }
    let visible = |value: &str| {
        let value = value.trim();
        value.eq_ignore_ascii_case("visible") || value.eq_ignore_ascii_case("auto")
    };
    !(node.attributes.get("overflow").is_some_and(|v| visible(v))
        || inline_style_prop(node, "overflow").is_some_and(visible))
}

/** The data URL prefix of a drawing an img shows; the coordinator writes it percent-escaped. */
const SVG_DATA: &str = "data:image/svg+xml,";

/// The vector paint of an img whose source is an `image/svg+xml` data URL, fitted into its
/// object rectangle (the content box after `object-fit`) by the drawing's own viewBox and
/// `preserveAspectRatio`, as Blink sizes an `SVGImage` to its container. Eye parses the markup itself; a drawing that does not
/// parse paints nothing, like a broken image. Base64 drawings are not read: the coordinator
/// rewrites every drawing it admits to this percent-escaped form.
pub(super) fn image_paths(
    context: &mut LayoutContext<'_>,
    source: &str,
    content: (f64, f64, f64, f64),
    current: &str,
) -> EngineResult<Vec<VectorPath>> {
    let Some(markup) = source
        .get(..SVG_DATA.len())
        .filter(|prefix| prefix.eq_ignore_ascii_case(SVG_DATA))
        .map(|_| percent_decode(&source[SVG_DATA.len()..]))
    else {
        return Ok(Vec::new());
    };
    let Ok(document) = crate::document::parse(&markup) else {
        return Ok(Vec::new());
    };
    let Some(svg) = document.nodes.iter().find(|node| node.tag == "svg") else {
        return Ok(Vec::new());
    };
    let mut svg = svg.clone();
    synthesize_view_box(&mut svg);
    drawing(
        &mut |work| context.charge(work),
        &document,
        &svg,
        content,
        current,
    )
}

/// Segments one background drawing may emit, so a hostile sprite cannot stall the painter.
const IMAGE_WORK: usize = 1 << 16;

impl VectorPath {
    /// The paint of an SVG document drawn as a CSS image (a background layer) into `tile`
    /// ([x, y, width, height]), fitted by its viewBox and `preserveAspectRatio` as Blink's
    /// `SVGImage` draws into its concrete object size. Markup that does not parse, or passes
    /// IMAGE_WORK segments, paints nothing, like a broken image.
    pub fn from_image(markup: &str, [x, y, width, height]: [f64; 4]) -> Vec<VectorPath> {
        let Ok(document) = crate::document::parse(markup) else {
            return Vec::new();
        };
        let Some(svg) = document.nodes.iter().find(|node| node.tag == "svg") else {
            return Vec::new();
        };
        let mut svg = svg.clone();
        synthesize_view_box(&mut svg);
        let mut left = IMAGE_WORK;
        let mut charge = |work: usize| {
            left = left
                .checked_sub(work)
                .ok_or_else(|| EngineError::limit("image work", IMAGE_WORK))?;
            Ok(())
        };
        drawing(
            &mut charge,
            &document,
            &svg,
            (x, y, width, height),
            "#000000",
        )
        .unwrap_or_default()
    }
}

/** An SVG image without a `viewBox` scales its `width` by `height` canvas into the img box, as
 * Chromium synthesizes a view box for SVG drawn as an image (rustdoc's 106px logo painted at
 * full size over a 48px box). Inline svg keeps CSS's unscaled user space. */
fn synthesize_view_box(svg: &mut Node) {
    if view_box_numbers(svg).is_some() {
        return;
    }
    let length = |name: &str| {
        let value = svg.attributes.get(name)?.trim();
        let value = value.strip_suffix("px").unwrap_or(value);
        value
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite() && *v > 0.0)
    };
    if let (Some(width), Some(height)) = (length("width"), length("height")) {
        svg.attributes
            .insert("viewbox".into(), format!("0 0 {width} {height}"));
    }
}

/** URL percent-decoding to UTF-8 text; a `%` without two hex digits stays literal. */
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while let Some(&byte) = bytes.get(at) {
        let hex = bytes
            .get(at + 1..at + 3)
            .filter(|pair| byte == b'%' && pair.iter().all(u8::is_ascii_hexdigit))
            .and_then(|pair| u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok());
        out.push(hex.unwrap_or(byte));
        at += if hex.is_some() { 3 } else { 1 };
    }
    String::from_utf8_lossy(&out).into_owned()
}

/** One path or `use` copy as vector paint with its segment count; None for other elements. */
fn shape(
    document: &Document,
    svg: &Node,
    node: &Node,
    viewport: Affine,
    current: &str,
) -> EngineResult<Option<(VectorPath, usize)>> {
    // `use` copies are drawn by `use_copies`.
    let (shape, inner) = match node.tag.as_str() {
        tag if DRAWN.contains(&tag) => (node, IDENTITY),
        _ => return Ok(None),
    };
    let mut chain = vec![node];
    chain.extend(ancestors(document, node, svg.local_id)?);
    let mut matrix = inner;
    for element in &chain {
        matrix = multiply(transform(element)?, matrix);
    }
    let mut paint = chain.clone();
    paint.push(svg);
    let segments = parse(&path_data(shape, view_box_numbers(svg))?)?;
    let path = vector(
        document,
        &segments,
        multiply(viewport, matrix),
        &paint,
        current,
        viewport[0],
    )?;
    Ok(Some((path, segments.len())))
}

/** Elements that paint a shape: path and the basic shapes, which become path data. */
const DRAWN: [&str; 7] = [
    "path", "rect", "circle", "ellipse", "line", "polyline", "polygon",
];

/// The path data a shape element stands for, per SVG 2's equivalent paths: a rect (with rx/ry
/// as arcs), circle and ellipse as two arcs, line, polyline and polygon from their points. A
/// missing length is 0; a malformed one drops the shape. A percentage resolves against the
/// root's viewBox (its user-space viewport): width for x, height for y, and the normalized
/// diagonal for radii; without a viewBox it drops the shape.
fn path_data(node: &Node, view: Option<[f64; 4]>) -> EngineResult<String> {
    let length = |name: &str| -> EngineResult<f64> {
        let Some(value) = node.attributes.get(name) else {
            return Ok(0.0);
        };
        let value = value.trim();
        if let Some(percent) = value.strip_suffix('%') {
            let [_, _, w, h] = view.ok_or_else(unsupported)?;
            let basis = match name {
                "x" | "width" | "cx" | "rx" | "x1" | "x2" => w,
                "y" | "height" | "cy" | "ry" | "y1" | "y2" => h,
                _ => ((w * w + h * h) / 2.0).sqrt(),
            };
            let percent = percent.trim().parse::<f64>().ok().filter(|v| v.is_finite());
            return percent.map(|p| p * basis / 100.0).ok_or_else(unsupported);
        }
        value
            .strip_suffix("px")
            .unwrap_or(value)
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
            .ok_or_else(unsupported)
    };
    Ok(match node.tag.as_str() {
        "rect" => {
            let (x, y, w, h) = (
                length("x")?,
                length("y")?,
                length("width")?,
                length("height")?,
            );
            let (rx, ry) = corner_radii(node, length("rx")?, length("ry")?, w, h);
            if rx > 0.0 && ry > 0.0 {
                format!(
                    "M{} {y}H{}A{rx} {ry} 0 0 1 {} {}V{}A{rx} {ry} 0 0 1 {} {}H{}A{rx} {ry} 0 0 1 {x} {}V{}A{rx} {ry} 0 0 1 {} {y}Z",
                    x + rx,
                    x + w - rx,
                    x + w,
                    y + ry,
                    y + h - ry,
                    x + w - rx,
                    y + h,
                    x + rx,
                    y + h - ry,
                    y + ry,
                    x + rx
                )
            } else {
                format!("M{x} {y}H{}V{}H{x}Z", x + w, y + h)
            }
        }
        "circle" => ellipse(length("cx")?, length("cy")?, length("r")?, length("r")?),
        "ellipse" => ellipse(length("cx")?, length("cy")?, length("rx")?, length("ry")?),
        "line" => format!(
            "M{} {}L{} {}",
            length("x1")?,
            length("y1")?,
            length("x2")?,
            length("y2")?
        ),
        "polyline" | "polygon" => {
            let points = node
                .attributes
                .get("points")
                .map(String::as_str)
                .unwrap_or("");
            let close = if node.tag == "polygon" { "Z" } else { "" };
            format!("M{}{close}", points.trim())
        }
        _ => node.attributes.get("d").cloned().unwrap_or_default(),
    })
}

/** A rect's used corner radii: one given radius stands for both, each at most half the side. */
fn corner_radii(node: &Node, rx: f64, ry: f64, width: f64, height: f64) -> (f64, f64) {
    let (has_x, has_y) = (
        node.attributes.contains_key("rx"),
        node.attributes.contains_key("ry"),
    );
    let (rx, ry) = match (has_x, has_y) {
        (true, false) => (rx, rx),
        (false, true) => (ry, ry),
        _ => (rx, ry),
    };
    (rx.clamp(0.0, width / 2.0), ry.clamp(0.0, height / 2.0))
}

/** An ellipse as two half arcs; a non-positive radius draws nothing. */
fn ellipse(cx: f64, cy: f64, rx: f64, ry: f64) -> String {
    if rx <= 0.0 || ry <= 0.0 {
        return String::new();
    }
    format!(
        "M{} {cy}A{rx} {ry} 0 1 0 {} {cy}A{rx} {ry} 0 1 0 {} {cy}Z",
        cx - rx,
        cx + rx,
        cx - rx
    )
}

/** Noted when render drops one SVG shape it cannot draw. */
const DROPPED_SHAPE: &str = "An SVG shape Eye cannot draw yet was skipped.";
/** Noted when an svg's size or viewBox is malformed and ignored. */
const IGNORED_SIZE: &str = "A malformed SVG width, height or viewBox was ignored.";

/// The value-free diagnostics for what render will drop or ignore in this document's drawings,
/// found by running the same per-shape checks render runs.
pub(super) fn notes(document: &Document) -> Vec<&'static str> {
    let mut notes = Vec::new();
    for svg in document.nodes.iter().filter(|node| node.tag == "svg") {
        let malformed_size = ["width", "height"]
            .iter()
            .any(|name| svg.attributes.contains_key(*name) && dimension(svg, name).is_none())
            || svg.attributes.contains_key("viewbox") && view_box_numbers(svg).is_none();
        if malformed_size && !notes.contains(&IGNORED_SIZE) {
            notes.push(IGNORED_SIZE);
        }
        let members = descendants(document, svg.local_id);
        let dropped = members.iter().any(|node| match node.tag.as_str() {
            "use" => use_copies(document, svg, node, IDENTITY, "#000000").is_err(),
            _ => shape(document, svg, node, IDENTITY, "#000000").is_err(),
        });
        if dropped && !notes.contains(&DROPPED_SHAPE) {
            notes.push(DROPPED_SHAPE);
        }
    }
    notes
}

/** Strict svg descendants in preorder; the parser keeps them contiguous. */
fn descendants(document: &Document, svg: usize) -> Vec<&Node> {
    let ids: std::collections::BTreeSet<usize> = mars_core::dom::descendant_ids(
        document
            .nodes
            .iter()
            .map(|node| (node.local_id, node.parent)),
        svg,
    )
    .into_iter()
    .collect();
    document
        .nodes
        .iter()
        .filter(|node| ids.contains(&node.local_id))
        .collect()
}

/** Ancestors strictly between a node and its svg, innermost first. */
fn ancestors<'a>(document: &'a Document, node: &Node, svg: usize) -> EngineResult<Vec<&'a Node>> {
    let mut result = Vec::new();
    let mut parent = node.parent;
    while let Some(id) = parent.filter(|id| *id != svg) {
        let ancestor = mars_core::dom::local_position(&document.nodes, id, |node| node.local_id)
            .map(|position| &document.nodes[position])
            .ok_or_else(|| EngineError::invalid("Invalid layout node identity."))?;
        result.push(ancestor);
        parent = ancestor.parent;
    }
    Ok(result)
}

/** A `use` copy translates by its x and y before any enclosing transform. */
fn use_offset(node: &Node) -> EngineResult<Affine> {
    let coordinate = |name: &str| -> EngineResult<f64> {
        node.attributes
            .get(name)
            .map(|value| value.trim().parse::<f64>().ok().filter(|v| v.is_finite()))
            .map(|value| value.ok_or_else(unsupported))
            .transpose()
            .map(|value| value.unwrap_or(0.0))
    };
    Ok([1.0, 0.0, 0.0, 1.0, coordinate("x")?, coordinate("y")?])
}

/** An SVG transform list: matrix, translate, scale, rotate, skewX and skewY, composed left to right. */
fn transform(node: &Node) -> EngineResult<Affine> {
    let Some(source) = node.attributes.get("transform") else {
        return Ok(IDENTITY);
    };
    let mut matrix = IDENTITY;
    let mut rest = source.trim();
    for _ in 0..TRANSFORM_FUNCTIONS {
        rest = rest.trim_start_matches(|c: char| c == ',' || c.is_ascii_whitespace());
        if rest.is_empty() {
            return finite(matrix);
        }
        let open = rest.find('(').ok_or_else(unsupported)?;
        let close = rest.find(')').ok_or_else(unsupported)?;
        if close < open {
            return Err(unsupported());
        }
        let name = rest[..open].trim().to_ascii_lowercase();
        let arguments = numbers(&rest[open + 1..close])?;
        matrix = multiply(matrix, function(&name, &arguments)?);
        rest = &rest[close + 1..];
    }
    Err(unsupported())
}
/** Transform functions one SVG `transform` attribute composes at most. */
const TRANSFORM_FUNCTIONS: usize = 16;
/** Matrix entries stay inside this magnitude so composed geometry remains bounded. */
const TRANSFORM_LIMIT: f64 = 1.0e6;
/** Rejects transforms whose entries overflow the bound. */
fn finite(matrix: Affine) -> EngineResult<Affine> {
    if matrix
        .iter()
        .all(|v| v.is_finite() && v.abs() <= TRANSFORM_LIMIT)
    {
        Ok(matrix)
    } else {
        Err(unsupported())
    }
}
/** One transform function's matrix. */
fn function(name: &str, arguments: &[f64]) -> EngineResult<Affine> {
    Ok(match (name, arguments) {
        ("matrix", [a, b, c, d, e, f]) => [*a, *b, *c, *d, *e, *f],
        ("translate", [x]) => [1.0, 0.0, 0.0, 1.0, *x, 0.0],
        ("translate", [x, y]) => [1.0, 0.0, 0.0, 1.0, *x, *y],
        ("scale", [s]) => [*s, 0.0, 0.0, *s, 0.0, 0.0],
        ("scale", [x, y]) => [*x, 0.0, 0.0, *y, 0.0, 0.0],
        ("rotate", [angle]) => rotation(*angle, 0.0, 0.0),
        ("rotate", [angle, cx, cy]) => rotation(*angle, *cx, *cy),
        ("skewx", [angle]) => [1.0, 0.0, angle.to_radians().tan(), 1.0, 0.0, 0.0],
        ("skewy", [angle]) => [1.0, angle.to_radians().tan(), 0.0, 1.0, 0.0, 0.0],
        _ => return Err(unsupported()),
    })
}
/** Rotation by `angle` degrees about (cx, cy). */
fn rotation(angle: f64, cx: f64, cy: f64) -> Affine {
    let (sin, cos) = angle.to_radians().sin_cos();
    [
        cos,
        sin,
        -sin,
        cos,
        cx - cos * cx + sin * cy,
        cy - sin * cx - cos * cy,
    ]
}

/// The viewBox-to-viewport transform of SVG's `preserveAspectRatio`, a port of Blink's
/// `SVGPreserveAspectRatio::ComputeTransform`: `none` scales each axis on its own; otherwise one
/// scale (the smaller for `meet`, the larger for `slice`) and the box aligned by its
/// x/y Min, Mid or Max edge. Index 0 doubles as the stroke scale. A missing or malformed viewBox
/// maps user units straight onto the viewport.
fn view_box(svg: &Node, viewport: (f64, f64, f64, f64)) -> Affine {
    match view_box_numbers(svg) {
        Some(numbers) => aspect_transform(numbers, aspect_ratio(svg), viewport),
        None => [1.0, 0.0, 0.0, 1.0, viewport.0, viewport.1],
    }
}
/** One axis alignment of `preserveAspectRatio`, as a fraction of the leftover room. */
type Align = Option<(f64, f64)>;
/** `preserveAspectRatio` as (alignment, slice): None alignment is `none`; the initial value is
 * `xMidYMid meet`, and a malformed value keeps it, as an invalid attribute does. */
fn aspect_ratio(svg: &Node) -> (Align, bool) {
    const MID: (Align, bool) = (Some((0.5, 0.5)), false);
    let Some(value) = svg.attributes.get("preserveaspectratio") else {
        return MID;
    };
    let mut words = value.split_whitespace();
    let mut align = words.next().unwrap_or("xMidYMid");
    if align == "defer" {
        align = words.next().unwrap_or("xMidYMid");
    }
    let slice = match words.next() {
        None | Some("meet") => false,
        Some("slice") => true,
        Some(_) => return MID,
    };
    if align == "none" {
        return (None, slice);
    }
    let fraction = |part: &str| match part {
        "Min" => Some(0.0),
        "Mid" => Some(0.5),
        "Max" => Some(1.0),
        _ => None,
    };
    let parsed = align
        .strip_prefix('x')
        .and_then(|rest| Some((fraction(rest.get(..3)?)?, rest.get(3..)?)))
        .and_then(|(x, rest)| Some((x, fraction(rest.strip_prefix('Y')?)?)));
    match parsed {
        Some(pair) => (Some(pair), slice),
        None => MID,
    }
}
/** The transform placing viewBox `[min_x, min_y, width, height]` into a viewport rectangle. */
fn aspect_transform(
    [min_x, min_y, box_width, box_height]: [f64; 4],
    (align, slice): (Align, bool),
    (x, y, width, height): (f64, f64, f64, f64),
) -> Affine {
    let (sx, sy) = (width / box_width, height / box_height);
    let Some((ax, ay)) = align else {
        return [sx, 0.0, 0.0, sy, x - min_x * sx, y - min_y * sy];
    };
    let scale = if slice { sx.max(sy) } else { sx.min(sy) };
    [
        scale,
        0.0,
        0.0,
        scale,
        x + (width - box_width * scale) * ax - min_x * scale,
        y + (height - box_height * scale) * ay - min_y * scale,
    ]
}

/** Four finite numbers with a positive width and height, or None. */
fn view_box_numbers(svg: &Node) -> Option<[f64; 4]> {
    let numbers = <[f64; 4]>::try_from(numbers(svg.attributes.get("viewbox")?).ok()?).ok()?;
    (numbers[2] > 0.0 && numbers[3] > 0.0).then_some(numbers)
}

/** Comma- or space-separated finite numbers. */
fn numbers(source: &str) -> EngineResult<Vec<f64>> {
    source
        .split(|c: char| c == ',' || c.is_ascii_whitespace())
        .filter(|part| !part.is_empty())
        .map(|part| {
            part.parse::<f64>()
                .ok()
                .filter(|value| value.is_finite())
                .ok_or_else(unsupported)
        })
        .collect()
}

/** Composes `outer` after `inner`. */
fn multiply(outer: Affine, inner: Affine) -> Affine {
    let [a, b, c, d, e, f] = outer;
    let [g, h, i, j, k, l] = inner;
    [
        a * g + c * h,
        b * g + d * h,
        a * i + c * j,
        b * i + d * j,
        a * k + c * l + e,
        b * k + d * l + f,
    ]
}

/** Maps one user-space point to document coordinates. */
fn apply(matrix: Affine, x: f64, y: f64) -> (f64, f64) {
    let [a, b, c, d, e, f] = matrix;
    (a * x + c * y + e, b * x + d * y + f)
}

/**
 * Paint properties inherit through the element chain, nearest first, then fall back to SVG
 * defaults. On one element the inline style wins, then stylesheet CSS, then the presentation
 * attribute, as the cascade ranks them.
 */
fn vector(
    document: &Document,
    segments: &[Segment],
    matrix: Affine,
    chain: &[&Node],
    current: &str,
    scale: f64,
) -> EngineResult<VectorPath> {
    let inherited = |name: &str| {
        chain.iter().find_map(|node| {
            inline_style_prop(node, name)
                .or_else(|| {
                    node.style
                        .svg_paint
                        .get(name)
                        .map(String::as_str)
                        .filter(|value| resolvable(document, value))
                })
                .or_else(|| node.attributes.get(name).map(String::as_str))
        })
    };
    let width = match inherited("stroke-width") {
        Some(value) => value
            .trim()
            .trim_end_matches("px")
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite() && (0.0..=CSS_MAX_SCALAR).contains(value))
            .ok_or_else(unsupported)?,
        None => 1.0,
    };
    let line_join = match inherited("stroke-linejoin").map(str::trim) {
        None | Some("miter") => LineJoin::Miter,
        Some("round") => LineJoin::Round,
        Some("bevel") => LineJoin::Bevel,
        Some(_) => return Err(unsupported()),
    };
    let line_cap = match inherited("stroke-linecap").map(str::trim) {
        None | Some("butt") => LineCap::Butt,
        Some("round") => LineCap::Round,
        Some("square") => LineCap::Square,
        Some(_) => return Err(unsupported()),
    };
    let fill_rule = match inherited("fill-rule").map(str::trim) {
        None | Some("nonzero") => FillRule::NonZero,
        Some("evenodd") => FillRule::EvenOdd,
        Some(_) => return Err(unsupported()),
    };
    let parse_opacity = |value: Option<&str>| -> f64 {
        let Some(raw) = value.map(str::trim) else {
            return 1.0;
        };
        if let Some(pct) = raw.strip_suffix('%') {
            pct.trim()
                .parse::<f64>()
                .ok()
                .filter(|v| v.is_finite())
                .map_or(1.0, |v| (v / 100.0).clamp(0.0, 1.0))
        } else {
            raw.parse::<f64>()
                .ok()
                .filter(|v| v.is_finite())
                .map_or(1.0, |v| v.clamp(0.0, 1.0))
        }
    };
    let group_opacity: f64 = chain
        .iter()
        .map(|node| {
            let attr = inline_style_prop(node, "opacity")
                .or_else(|| node.attributes.get("opacity").map(String::as_str));
            parse_opacity(attr)
        })
        .product::<f64>()
        * chain
            .iter()
            .map(|node| mask_coverage(document, node, current))
            .product::<f64>();
    let fill_opacity = (group_opacity * parse_opacity(inherited("fill-opacity"))).clamp(0.0, 1.0);
    let stroke_opacity =
        (group_opacity * parse_opacity(inherited("stroke-opacity"))).clamp(0.0, 1.0);
    Ok(VectorPath {
        commands: segments
            .iter()
            .map(|segment| command(*segment, matrix))
            .collect(),
        fill: paint(document, inherited("fill").unwrap_or("black"), current)?,
        stroke: paint(document, inherited("stroke").unwrap_or("none"), current)?,
        stroke_width: width * scale,
        line_join,
        line_cap,
        fill_rule,
        fill_opacity,
        stroke_opacity,
    })
}

/**
 * How much of a masked element shows, as one factor on its opacity: SVG Masking's luminance or
 * alpha coverage of the referenced `<mask>`, averaged over its content's solid or mean gradient
 * paint (Blink's SVGMaskPainter composites the mask image; one mean value stands in for it). 1.0
 * when the element has no `mask` or names none Eye can read.
 * ponytail: the mean of the first painted shape, not a per-pixel mask; a mask image on
 * `VectorPath` that the desktop painter applies is the upgrade.
 */
fn mask_coverage(document: &Document, node: &Node, current: &str) -> f64 {
    let Some(id) = inline_style_prop(node, "mask")
        .or_else(|| node.attributes.get("mask").map(String::as_str))
        .and_then(|value| value.trim().strip_prefix("url(#"))
        .and_then(|rest| rest.split_once(')'))
        .map(|(id, _)| id.trim_matches(['"', '\'']))
    else {
        return 1.0;
    };
    let Some(mask) = document.nodes.iter().find(|candidate| {
        candidate.tag == "mask"
            && candidate
                .attributes
                .get("id")
                .is_some_and(|value| value == id)
    }) else {
        return 1.0;
    };
    let alpha_only = inline_style_prop(mask, "mask-type")
        .or_else(|| mask.attributes.get("mask-type").map(String::as_str))
        .is_some_and(|kind| kind.trim().eq_ignore_ascii_case("alpha"));
    let Some(fill) = document
        .nodes
        .iter()
        .filter(|child| child.parent == Some(mask.local_id))
        .map(|child| {
            inline_style_prop(child, "fill")
                .or_else(|| child.attributes.get("fill").map(String::as_str))
                .unwrap_or("black")
        })
        .next()
    else {
        // An empty mask hides the element entirely.
        return 0.0;
    };
    let Some([r, g, b, a]) = paint(document, fill, current)
        .ok()
        .flatten()
        .and_then(|literal| channels(&literal))
    else {
        return 0.0;
    };
    if alpha_only {
        a
    } else {
        a * (LUMINANCE[0] * r + LUMINANCE[1] * g + LUMINANCE[2] * b)
    }
}
/** The luminance weights SVG Masking gives `mask-type: luminance` (sRGB coefficients). */
const LUMINANCE: [f64; 3] = [0.2125, 0.7154, 0.0721];

/** Extracts an inline `style="prop: value"` property from an SVG element when present. */
fn inline_style_prop<'a>(node: &'a Node, prop: &str) -> Option<&'a str> {
    let style = node.attributes.get("style")?;
    let mut matched = None;
    for decl in style.split(';') {
        let Some((name, value)) = decl.split_once(':') else {
            continue;
        };
        if name.trim().eq_ignore_ascii_case(prop) {
            let value = value.trim();
            let value = value.strip_suffix("!important").map_or(value, str::trim);
            if !value.is_empty() {
                matched = Some(value);
            }
        }
    }
    matched
}

/** One user-space segment in document coordinates. */
fn command(segment: Segment, matrix: Affine) -> PathCommand {
    match segment {
        Segment::Move(x, y) => {
            let (x, y) = apply(matrix, x, y);
            PathCommand::Move { x, y }
        }
        Segment::Line(x, y) => {
            let (x, y) = apply(matrix, x, y);
            PathCommand::Line { x, y }
        }
        Segment::Cubic([x1, y1, x2, y2, x, y]) => {
            let (x1, y1) = apply(matrix, x1, y1);
            let (x2, y2) = apply(matrix, x2, y2);
            let (x, y) = apply(matrix, x, y);
            PathCommand::Cubic {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            }
        }
        Segment::Close => PathCommand::Close,
    }
}

/** `none` paints nothing, `currentColor` is the svg's used text color, literals use the CSS
 * validator, and `url(#id) [fallback]` resolves a paint server as Blink's
 * `SVGResources::GetPaint` does: a gradient paints, and a missing or unusable reference paints
 * the fallback. */
fn paint(document: &Document, value: &str, current: &str) -> EngineResult<Option<String>> {
    let value = value.trim();
    if value
        .get(..4)
        .is_some_and(|head| head.eq_ignore_ascii_case("url("))
    {
        let close = value.find(')').ok_or_else(unsupported)?;
        let reference = value[4..close].trim().trim_matches(['"', '\'']);
        let fallback = value[close + 1..].trim();
        if let Some(color) = reference
            .strip_prefix('#')
            .and_then(|id| gradient_color(document, id))
        {
            return Ok(color);
        }
        // A missing server without a fallback paints nothing in Blink; the shape is dropped
        // with the unsupported-shape note instead, since the parser keeps no pattern, mask or
        // clip content for Eye to tell an author error from a server it cannot draw.
        return match fallback {
            "" => Err(unsupported()),
            fallback => paint(document, fallback, current),
        };
    }
    let value = value.to_ascii_lowercase();
    match value.as_str() {
        "none" => Ok(None),
        "currentcolor" => Ok(Some(current.into())),
        _ => crate::document::style::literal_color(&value).map(Some),
    }
}

/** A stylesheet `url(#id)` paint without a fallback whose gradient is missing is dropped like
 * an invalid declaration, so the shape keeps its presentation attribute paint. */
fn resolvable(document: &Document, value: &str) -> bool {
    let Some(rest) = value
        .get(..5)
        .filter(|head| head.eq_ignore_ascii_case("url(#"))
        .map(|_| &value[5..])
    else {
        return true;
    };
    let Some((id, fallback)) = rest.split_once(')') else {
        return false;
    };
    !fallback.trim().is_empty() || gradient_color(document, id.trim_matches(['"', '\''])).is_some()
}
/** Gradients a paint server's `href` chain follows for inherited stops before giving up. */
const GRADIENT_HREFS: usize = 8;
/**
 * The paint of a `linearGradient` or `radialGradient` by id: Some(None) for one without stops
 * (SVG paints nothing), its only stop's color, or else its mean color along the gradient, the
 * average of the piecewise-linear ramp with premultiplied alpha. None when `id` names no gradient.
 * ponytail: one solid color stands in for the ramp because `VectorPath` carries solid paint
 * only; a gradient field on `VectorPath` that the desktop painter draws is the upgrade.
 */
fn gradient_color(document: &Document, id: &str) -> Option<Option<String>> {
    let by_id = |id: &str| {
        document.nodes.iter().find(|node| {
            node.html_id == id || node.attributes.get("id").is_some_and(|value| value == id)
        })
    };
    let mut gradient = by_id(id).filter(|node| GRADIENTS.contains(&node.tag.as_str()))?;
    for _ in 0..GRADIENT_HREFS {
        let stops = gradient_stops(document, gradient);
        if !stops.is_empty() {
            return Some(mean_color(&stops));
        }
        let Some(next) = ["href", "xlink:href"]
            .iter()
            .find_map(|name| gradient.attributes.get(*name))
            .and_then(|href| href.trim().strip_prefix('#'))
            .and_then(by_id)
            .filter(|node| GRADIENTS.contains(&node.tag.as_str()))
        else {
            break;
        };
        gradient = next;
    }
    Some(None)
}
/** Paint servers Eye resolves to a color. */
const GRADIENTS: [&str; 2] = ["lineargradient", "radialgradient"];
/** A gradient's stops as (offset, [r, g, b, a]) with offsets clamped to [0, 1] and kept
 * nondecreasing, as SVG orders them. */
fn gradient_stops(document: &Document, gradient: &Node) -> Vec<(f64, [f64; 4])> {
    let mut floor: f64 = 0.0;
    let mut stops = Vec::new();
    for stop in document
        .nodes
        .iter()
        .filter(|node| node.parent == Some(gradient.local_id) && node.tag == "stop")
    {
        let property = |name: &str| {
            inline_style_prop(stop, name).or_else(|| stop.attributes.get(name).map(String::as_str))
        };
        let offset = property("offset").map_or(0.0, |value| {
            let value = value.trim();
            match value.strip_suffix('%') {
                Some(percent) => percent.trim().parse::<f64>().map_or(0.0, |p| p / 100.0),
                None => value.parse::<f64>().unwrap_or(0.0),
            }
        });
        floor = floor.max(if offset.is_finite() {
            offset.clamp(0.0, 1.0)
        } else {
            0.0
        });
        let color = property("stop-color")
            .unwrap_or("black")
            .trim()
            .to_ascii_lowercase();
        let Some(mut rgba) = crate::document::style::literal_color(&color)
            .ok()
            .and_then(|literal| channels(&literal))
        else {
            continue;
        };
        let opacity = property("stop-opacity")
            .and_then(|value| value.trim().parse::<f64>().ok())
            .filter(|value| value.is_finite())
            .map_or(1.0, |value| value.clamp(0.0, 1.0));
        rgba[3] *= opacity;
        stops.push((floor, rgba));
    }
    stops
}
/** The CSS values of the color names the style validator keeps as names. */
const PALETTE_HEX: [(&str, &str); 9] = [
    ("black", "#000000"),
    ("white", "#ffffff"),
    ("red", "#ff0000"),
    ("green", "#008000"),
    ("blue", "#0000ff"),
    ("gray", "#808080"),
    ("grey", "#808080"),
    ("navy", "#000080"),
    ("teal", "#008080"),
];
/** RGBA channels in 0..1 of a hex, palette name or `transparent` literal. */
pub(super) fn channels(literal: &str) -> Option<[f64; 4]> {
    if literal == "transparent" {
        return Some([0.0; 4]);
    }
    let named = PALETTE_HEX
        .iter()
        .find(|(name, _)| *name == literal)
        .map(|(_, hex)| *hex);
    let digits = named.unwrap_or(literal).strip_prefix('#')?;
    let expanded: String = match digits.len() {
        3 => digits.chars().flat_map(|c| [c, c]).collect(),
        6 | 8 => digits.into(),
        _ => return None,
    };
    let byte = |at: usize| {
        expanded
            .get(at..at + 2)
            .and_then(|pair| u8::from_str_radix(pair, 16).ok())
    };
    let alpha = if expanded.len() == 8 {
        byte(6)?
    } else {
        u8::MAX
    };
    Some([byte(0)?, byte(2)?, byte(4)?, alpha].map(|v| f64::from(v) / f64::from(u8::MAX)))
}
/** The mean of the ramp the stops describe over [0, 1], padded with the end colors. */
fn mean_color(stops: &[(f64, [f64; 4])]) -> Option<String> {
    let premultiplied = |[r, g, b, a]: [f64; 4]| [r * a, g * a, b * a, a];
    let mut points: Vec<(f64, [f64; 4])> = stops
        .iter()
        .map(|(at, c)| (*at, premultiplied(*c)))
        .collect();
    let (first, last) = (*points.first()?, *points.last()?);
    points.insert(0, (0.0, first.1));
    points.push((1.0, last.1));
    let mut sum = [0.0; 4];
    for pair in points.windows(2) {
        let ((from, a), (to, b)) = (pair[0], pair[1]);
        for channel in 0..4 {
            sum[channel] += (to - from) * (a[channel] + b[channel]) / 2.0;
        }
    }
    Some(literal(sum))
}
/** The painter literal of premultiplied RGBA channels in 0..1. */
pub(super) fn literal([r, g, b, alpha]: [f64; 4]) -> String {
    if alpha <= 0.0 {
        return "transparent".into();
    }
    let byte = |value: f64| (value * f64::from(u8::MAX)).round().clamp(0.0, 255.0) as u8;
    let [r, g, b] = [r / alpha, g / alpha, b / alpha].map(byte);
    match byte(alpha) {
        u8::MAX => format!("#{r:02x}{g:02x}{b:02x}"),
        a => format!("#{r:02x}{g:02x}{b:02x}{a:02x}"),
    }
}

/** Unsupported drawing input names no author data. */
fn unsupported() -> EngineError {
    EngineError::unsupported("This page uses an SVG feature Eye does not support yet.")
}
