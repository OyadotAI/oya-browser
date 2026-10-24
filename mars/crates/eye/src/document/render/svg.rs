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
    if flow.cursor > 0.0 && flow.cursor + outer > flow.room() {
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
    drawing(context, context.document, svg, content, current)
}

/** The shapes of `svg`, a drawing inside `document`, charged to the layout's work budget. */
fn drawing(
    context: &mut LayoutContext<'_>,
    document: &Document,
    svg: &Node,
    content: (f64, f64, f64, f64),
    current: &str,
) -> EngineResult<Vec<VectorPath>> {
    let viewport = view_box(svg, content);
    let members = descendants(document, svg.local_id);
    let mut result = Vec::new();
    for node in &members {
        if let Ok(Some((path, segments))) = shape(document, svg, &members, node, viewport, current)
        {
            context.charge(segments + 1)?;
            result.push(path);
        }
    }
    Ok(result)
}

/** The data URL prefix of a drawing an img shows; the coordinator writes it percent-escaped. */
const SVG_DATA: &str = "data:image/svg+xml,";

/// The vector paint of an img whose source is an `image/svg+xml` data URL, fitted into its
/// content box the way inline svg is. Eye parses the markup itself; a drawing that does not
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
    drawing(context, &document, svg, content, current)
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
    members: &[&Node],
    node: &Node,
    viewport: Affine,
    current: &str,
) -> EngineResult<Option<(VectorPath, usize)>> {
    let (shape, inner) = match node.tag.as_str() {
        tag if DRAWN.contains(&tag) => (node, IDENTITY),
        "use" => {
            let shape = referenced(members, node)?;
            (shape, multiply(use_offset(node)?, transform(shape)?))
        }
        _ => return Ok(None),
    };
    let mut chain = vec![node];
    chain.extend(ancestors(document, node, svg.local_id)?);
    let mut matrix = inner;
    for element in &chain {
        matrix = multiply(transform(element)?, matrix);
    }
    let mut paint = if node.tag == "use" {
        vec![shape]
    } else {
        Vec::new()
    };
    paint.extend(&chain);
    paint.push(svg);
    let segments = parse(&path_data(shape)?)?;
    let path = vector(
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
/// missing length is 0; a malformed or percentage one drops the shape.
fn path_data(node: &Node) -> EngineResult<String> {
    let length = |name: &str| -> EngineResult<f64> {
        let Some(value) = node.attributes.get(name) else {
            return Ok(0.0);
        };
        let value = value.trim();
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
        let dropped = members
            .iter()
            .any(|node| shape(document, svg, &members, node, IDENTITY, "#000000").is_err());
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

/** `use` resolves only to a path in the same svg, so references never nest past `SVG_USE_DEPTH` (one). */
fn referenced<'a>(members: &[&'a Node], node: &Node) -> EngineResult<&'a Node> {
    let id = node
        .attributes
        .get("href")
        .and_then(|href| href.strip_prefix('#'))
        .ok_or_else(unsupported)?;
    let target = members
        .iter()
        .find(|member| member.attributes.get("id").is_some_and(|value| value == id))
        .ok_or_else(unsupported)?;
    if !DRAWN.contains(&target.tag.as_str()) {
        return Err(EngineError::unsupported(
            "SVG use may reference only a shape in the same drawing.",
        ));
    }
    Ok(target)
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

/// xMidYMid meet: one uniform scale, centred in the content box. Index 0 doubles as that scale.
/// A missing or malformed viewBox maps user units straight onto the content box.
fn view_box(svg: &Node, (x, y, width, height): (f64, f64, f64, f64)) -> Affine {
    let Some([min_x, min_y, box_width, box_height]) = view_box_numbers(svg) else {
        return [1.0, 0.0, 0.0, 1.0, x, y];
    };
    let scale = (width / box_width).min(height / box_height);
    [
        scale,
        0.0,
        0.0,
        scale,
        x + (width - box_width * scale) / 2.0 - min_x * scale,
        y + (height - box_height * scale) / 2.0 - min_y * scale,
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

/** Paint properties inherit through the element chain, nearest first, then fall back to SVG defaults. */
fn vector(
    segments: &[Segment],
    matrix: Affine,
    chain: &[&Node],
    current: &str,
    scale: f64,
) -> EngineResult<VectorPath> {
    let inherited = |name: &str| {
        chain.iter().find_map(|node| {
            inline_style_prop(node, name).or_else(|| node.attributes.get(name).map(String::as_str))
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
        .product();
    let fill_opacity = (group_opacity * parse_opacity(inherited("fill-opacity"))).clamp(0.0, 1.0);
    let stroke_opacity =
        (group_opacity * parse_opacity(inherited("stroke-opacity"))).clamp(0.0, 1.0);
    Ok(VectorPath {
        commands: segments
            .iter()
            .map(|segment| command(*segment, matrix))
            .collect(),
        fill: paint(inherited("fill").unwrap_or("black"), current)?,
        stroke: paint(inherited("stroke").unwrap_or("none"), current)?,
        stroke_width: width * scale,
        line_join,
        line_cap,
        fill_rule,
        fill_opacity,
        stroke_opacity,
    })
}

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

/** `none` paints nothing, `currentColor` is the svg's used text color, literals use the CSS validator. */
fn paint(value: &str, current: &str) -> EngineResult<Option<String>> {
    let value = value.trim().to_ascii_lowercase();
    match value.as_str() {
        "none" => Ok(None),
        "currentcolor" => Ok(Some(current.into())),
        _ => crate::document::style::literal_color(&value).map(Some),
    }
}

/** Unsupported drawing input names no author data. */
fn unsupported() -> EngineError {
    EngineError::unsupported("This page uses an SVG feature Eye does not support yet.")
}
