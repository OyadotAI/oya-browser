/** Shared sizing caches and work limits keep intrinsic, dry-run and final layout bounded. */
use super::validation::bounded;
use super::{DisplayItem, flow};
use crate::{
    constants::{LAYOUT_EXTENT, LAYOUT_MEASURES, LAYOUT_WORK},
    document::{BoxSizing, Document, FlexDirection, FlexWrap, Node},
    platform::{Face, Family, measure},
    protocol::{EngineError, EngineResult},
};
use std::collections::BTreeMap;

/** Assigned border boxes never reinterpret an item's authored width. */
#[derive(Clone, Copy, Debug)]
pub(super) struct BorderBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: Option<f64>,
}
/** Intrinsic widths exclude the node's own decorations and margins. */
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct IntrinsicWidths {
    pub min_content: f64,
    pub max_content: f64,
}
/** One request shares every measurement and recursive visit budget. */
pub(super) struct LayoutContext<'a> {
    pub document: &'a Document,
    /** Identity to vector position, built once so no lookup rescans the tree. */
    positions: Vec<Option<usize>>,
    /** Child positions per parent identity in vector order, hidden or not. */
    kids: Vec<Vec<usize>>,
    work: usize,
    /** Uncached native measurements left; each costs a fixed native call the work units undercount. */
    measures: usize,
    intrinsic: BTreeMap<usize, IntrinsicWidths>,
    heights: BTreeMap<(usize, u64), f64>,
    /** Cached widths are bounded by the same work needed to create each key. */
    measured: BTreeMap<(String, FaceKey), f64>,
    /** Completed word wrapping is reusable across dry runs with identical font and containing width. */
    wrapped: BTreeMap<(u64, FaceKey), BTreeMap<String, Vec<super::wrap::MeasuredText>>>,
    /** Out-of-flow boxes by identity with their latest static position, placed after normal flow. */
    deferred: BTreeMap<usize, (f64, f64)>,
    /** A bound ran out; the rest of the page is not laid out. */
    exhausted: bool,
    /** List item ordinals by identity, computed once per list. */
    ordinals: BTreeMap<usize, i64>,
}
impl<'a> LayoutContext<'a> {
    /** Validate identities before any recursive layout, including direct Rust callers. */
    pub fn new(document: &'a Document) -> EngineResult<Self> {
        super::validation::tree(document)?;
        let positions = positions(document);
        let mut kids = vec![Vec::new(); positions.len()];
        for (position, node) in document.nodes.iter().enumerate() {
            if let Some(parent) = node.parent {
                kids[parent].push(position);
            }
        }
        Ok(Self {
            document,
            positions,
            kids,
            work: LAYOUT_WORK,
            measures: LAYOUT_MEASURES,
            intrinsic: BTreeMap::new(),
            heights: BTreeMap::new(),
            measured: BTreeMap::new(),
            wrapped: BTreeMap::new(),
            deferred: BTreeMap::new(),
            exhausted: false,
            ordinals: BTreeMap::new(),
        })
    }
    /** Shared work charges include repeated measurement rather than resetting in a dry run. */
    pub fn charge(&mut self, amount: usize) -> EngineResult<()> {
        let Some(left) = self.work.checked_sub(amount) else {
            self.work = 0;
            self.exhausted = true;
            return Err(EngineError::limit("layout work", LAYOUT_WORK));
        };
        self.work = left;
        Ok(())
    }
    /** Whether a work or measurement bound ran out, so layout stopped early. */
    pub fn exhausted(&self) -> bool {
        self.exhausted
    }
    /** After exhaustion, a small fixed reserve finishes the lines and boxes already placed. */
    pub fn reserve(&mut self) {
        self.work = LAYOUT_RESERVE;
        self.measures = MEASURE_RESERVE;
    }
    /** Charge scans and native shaping before either can exceed the shared request allowance. */
    pub fn measure(&mut self, text: &str, face: Face) -> EngineResult<f64> {
        let scalars = text.chars().count();
        self.charge(scalars + 1)?;
        let key = (text.to_owned(), face_key(face));
        if let Some(width) = self.measured.get(&key) {
            return Ok(*width);
        }
        self.charge(scalars + 1)?;
        let Some(left) = self.measures.checked_sub(1) else {
            self.exhausted = true;
            return Err(EngineError::limit("layout measurements", LAYOUT_MEASURES));
        };
        self.measures = left;
        let width = measure(text, face);
        bounded(width)?;
        self.measured.insert(key, width);
        Ok(width)
    }
    /** A borrowed source lookup avoids rebuilding scalar boundaries and repeated native prefix probes. */
    pub fn wrapped(
        &mut self,
        text: &str,
        width: f64,
        face: Face,
    ) -> EngineResult<Option<Vec<super::wrap::MeasuredText>>> {
        let key = (width.to_bits(), face_key(face));
        self.charge(text.chars().count() + 1)?;
        Ok(self
            .wrapped
            .get(&key)
            .and_then(|runs| runs.get(text))
            .cloned())
    }
    /** Cached text and run copies are admitted against the same finite request allowance. */
    pub fn remember_wrapped(
        &mut self,
        text: &str,
        width: f64,
        face: Face,
        lines: &[super::wrap::MeasuredText],
    ) -> EngineResult<()> {
        self.charge(text.chars().count() * 2 + 1)?;
        let key = (width.to_bits(), face_key(face));
        self.wrapped
            .entry(key)
            .or_default()
            .insert(text.to_owned(), lines.to_vec());
        Ok(())
    }
    /** Records an absolute or fixed box at its static position; the final layout pass overwrites dry runs. */
    pub fn defer(&mut self, local: usize, position: (f64, f64)) {
        self.deferred.insert(local, position);
    }
    /** Takes every recorded out-of-flow box, in document order. */
    pub fn take_deferred(&mut self) -> Vec<(usize, (f64, f64))> {
        let mut taken = std::mem::take(&mut self.deferred)
            .into_iter()
            .collect::<Vec<_>>();
        taken.sort_by_key(|(local, _)| self.positions.get(*local).copied().flatten());
        taken
    }
    /** All lookup uses stable identities, never vector positions after descendant replacement. */
    pub fn node(&self, local: usize) -> EngineResult<&'a Node> {
        self.positions
            .get(local)
            .copied()
            .flatten()
            .map(|position| &self.document.nodes[position])
            .ok_or_else(|| EngineError::invalid("Invalid layout node identity."))
    }
    /** Source-order children share their parent's already validated tree depth. */
    pub fn children(&self, local: usize) -> Vec<&'a Node> {
        let mut children = self.all_children(local);
        children.retain(|node| !node.hidden());
        children
    }
    /**
     * Every direct child in source order, hidden ones included, for callers that skip them later.
     * A `display: contents` child is replaced by its own children, so they join this formatting
     * context; recursion is bounded by the validated tree depth.
     */
    pub fn all_children(&self, local: usize) -> Vec<&'a Node> {
        let mut out = Vec::new();
        for &i in self.kids.get(local).map(Vec::as_slice).unwrap_or_default() {
            let node = &self.document.nodes[i];
            if contents(node) {
                out.extend(self.all_children(node.local_id));
            } else {
                out.push(node);
            }
        }
        out
    }
    /** Strict descendants in vector order, costing the subtree's size instead of the whole tree. */
    pub fn descendants(&self, local: usize) -> Vec<&'a Node> {
        let mut found = Vec::new();
        let mut pending = self.kids.get(local).cloned().unwrap_or_default();
        while let Some(position) = pending.pop() {
            found.push(position);
            pending.extend(&self.kids[self.document.nodes[position].local_id]);
        }
        found.sort_unstable();
        found.into_iter().map(|i| &self.document.nodes[i]).collect()
    }
    /** Intrinsic measurement is memoized per stable node for the entire request. */
    pub fn intrinsic_widths(&mut self, local: usize) -> EngineResult<IntrinsicWidths> {
        if let Some(value) = self.intrinsic.get(&local) {
            return Ok(*value);
        }
        self.charge(1)?;
        let node = self.node(local)?;
        if node.tag == "table" {
            let size = super::table::intrinsic(self, node)?;
            self.intrinsic.insert(local, size);
            return Ok(size);
        }
        let mut size = text_intrinsic(self, node)?;
        let mut children = self.children(local);
        children.retain(|child| !flow::out_of_flow(child));
        let mut sum = if node.style.display_flex && !node.text.trim().is_empty() {
            size
        } else {
            IntrinsicWidths::default()
        };
        for child in &children {
            let value = self.intrinsic_widths(child.local_id)?;
            let contribution = intrinsic_contribution(node, child, value);
            let extra = horizontal_edges(child) + child.style.margin[1] + child.style.margin[3];
            let minimum = contribution.min_content;
            let max = contribution.max_content;
            size.min_content = size.min_content.max(minimum + extra);
            size.max_content = size.max_content.max(max + extra);
            sum.min_content += minimum + extra;
            sum.max_content += max + extra;
        }
        if node.style.display_flex && node.style.flex_direction == FlexDirection::Row {
            let items = children.len() + usize::from(!node.text.trim().is_empty());
            let gaps = node.style.column_gap * items.saturating_sub(1) as f64;
            size.max_content = size.max_content.max(sum.max_content + gaps);
            if node.style.flex_wrap == FlexWrap::NoWrap {
                size.min_content = size.min_content.max(sum.min_content + gaps);
            }
        } else if children.iter().all(|child| flow::inline_level(child)) {
            size.max_content = size.max_content.max(sum.max_content);
        }
        bounded(size.min_content)?;
        bounded(size.max_content)?;
        self.intrinsic.insert(local, size);
        Ok(size)
    }
    /** Dry runs reuse the same algorithm and budget, including controls and nested flex. */
    pub fn measure_height(&mut self, local: usize, width: f64) -> EngineResult<f64> {
        if let Some(height) = self.heights.get(&(local, width.to_bits())) {
            return Ok(*height);
        }
        let mut items = Vec::new();
        let height = self.layout_at(
            local,
            BorderBox {
                x: 0.0,
                y: 0.0,
                width,
                height: None,
            },
            &mut items,
        )?;
        self.heights.insert((local, width.to_bits()), height);
        Ok(height)
    }
    /** Final placement consumes allocated border dimensions directly. */
    pub fn layout_at(
        &mut self,
        local: usize,
        bounds: BorderBox,
        items: &mut Vec<DisplayItem>,
    ) -> EngineResult<f64> {
        let node = self.node(local)?;
        let start = items.len();
        let mut target = flow::Flow::new(bounds.x, bounds.y, bounds.width);
        flow::node_layout(self, node, &mut target, items, Some(bounds))?;
        Ok(items.get(start).map(|item| item.height).unwrap_or(0.0))
    }
}
impl LayoutContext<'_> {
    /** A list item's number: its list's `start` plus the list items before it, computed for the whole list once. */
    pub fn ordinal(&mut self, node: &Node) -> EngineResult<i64> {
        if let Some(ordinal) = self.ordinals.get(&node.local_id) {
            return Ok(*ordinal);
        }
        let Some(parent) = node.parent else {
            return Ok(1);
        };
        let mut next = self
            .node(parent)?
            .attributes
            .get("start")
            .and_then(|value| value.trim().parse::<i64>().ok())
            .unwrap_or(1);
        for sibling in self.children(parent) {
            self.charge(1)?;
            if sibling.style.list_item {
                self.ordinals.insert(sibling.local_id, next);
                next = next.saturating_add(1);
            }
        }
        Ok(self.ordinals.get(&node.local_id).copied().unwrap_or(1))
    }
}
/** Work left to finish placed content after a bound ran out. */
const LAYOUT_RESERVE: usize = crate::constants::NODES * 8;
/** Native measurements left to finish placed content after a bound ran out. */
const MEASURE_RESERVE: usize = 256;
/** Cache identity of a face: size and spacing bits, weight, family and posture. */
type FaceKey = (u64, u16, Family, u64, bool);
/** Floats key by their bits so identical faces share cached shaping. */
fn face_key(face: Face) -> FaceKey {
    (
        face.size.to_bits(),
        face.weight,
        face.family,
        face.spacing.to_bits(),
        face.italic,
    )
}
/** The face a computed style shapes its text with, for both measuring and painting. */
pub(super) fn face(style: &crate::document::ComputedStyle) -> Face {
    face_for(style, None)
}
/** The face for one text run: a loaded page font covering the run's first letter when the
style's family list names one, else the generic family. */
pub(super) fn face_for(style: &crate::document::ComputedStyle, text: Option<&str>) -> Face {
    use crate::document::FontFamily;
    let weight = if style.bold && style.font_weight < 600 {
        700
    } else {
        style.font_weight
    };
    let sample = text.and_then(|text| text.chars().find(|c| c.is_alphanumeric()));
    let web =
        crate::document::style::select_font(&style.font_families, weight, style.font_style, sample);
    Face {
        size: style.font_size,
        weight: web.map_or(weight, |(_, clamped)| clamped),
        family: match (web, style.font_family) {
            (Some((key, _)), _) => Family::Web(key),
            (None, FontFamily::Serif) => Family::Serif,
            (None, FontFamily::SansSerif) => Family::SansSerif,
            (None, FontFamily::Monospace) => Family::Monospace,
            (None, FontFamily::SystemUi) => Family::System,
        },
        spacing: style.letter_spacing,
        italic: style.font_style != crate::document::FontStyle::Normal,
    }
}
/// Distinct faces whose native metrics are cached; the cache resets rather than grow past this.
const ADVANCE_CACHE: usize = 256;
thread_local! {
    /** Native ascender and normal line height per face; asking AppKit for every word would dominate layout. */
    static METRICS: std::cell::RefCell<BTreeMap<FaceKey, (f64, f64)>> = const { std::cell::RefCell::new(BTreeMap::new()) };
}
/** The face's ascender and `line-height: normal`, finite and positive. */
fn face_metrics(style: &crate::document::ComputedStyle) -> (f64, f64) {
    let face = face(style);
    let key = face_key(face);
    METRICS.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(value) = cache.get(&key) {
            return *value;
        }
        if cache.len() >= ADVANCE_CACHE {
            cache.clear();
        }
        let (ascent, normal) = crate::platform::metrics(face);
        let normal = if normal.is_finite() && normal > 0.0 {
            normal
        } else {
            style.font_size
        };
        let ascent = if ascent.is_finite() && ascent > 0.0 {
            ascent.min(normal)
        } else {
            normal * 0.8
        };
        cache.insert(key, (ascent, normal));
        (ascent, normal)
    })
}
/** The used line advance: `normal` is the face's own line height, as Chromium computes it. */
pub(super) fn advance(style: &crate::document::ComputedStyle) -> f64 {
    if style.line_height != crate::document::LineHeight::Normal {
        return style.line_height.used(style.font_size);
    }
    face_metrics(style).1
}
/** Distance from a line's top to its text baseline: half the leading plus the ascender, as painters draw it. */
pub(super) fn text_ascent(style: &crate::document::ComputedStyle) -> f64 {
    let (ascent, normal) = face_metrics(style);
    ((advance(style) - normal) / 2.0).max(0.0) + ascent
}
/** Identity to vector position; identities are validated below NODES and unique before this runs. */
pub(super) fn positions(document: &Document) -> Vec<Option<usize>> {
    let size = document
        .nodes
        .iter()
        .map(|node| node.local_id + 1)
        .max()
        .unwrap_or(0);
    let mut positions = vec![None; size];
    for (position, node) in document.nodes.iter().enumerate() {
        positions[node.local_id] = Some(position);
    }
    positions
}
/** Painted border widths top, right, bottom, left; a uniform `border` wins over per-side values. */
pub(super) fn border_sides(style: &crate::document::ComputedStyle) -> [f64; 4] {
    if style.border > 0.0 {
        [style.border; 4]
    } else {
        style.border_widths
    }
}
/** Decorations are always excluded from flexible content sizes. */
pub(super) fn horizontal_edges(node: &Node) -> f64 {
    let sides = border_sides(&node.style);
    node.style.padding[1] + node.style.padding[3] + sides[1] + sides[3]
}
/** Auto-height columns use the actual block-axis border/padding extent. */
pub(super) fn vertical_edges(node: &Node) -> f64 {
    let sides = border_sides(&node.style);
    node.style.padding[0] + node.style.padding[2] + sides[0] + sides[2]
}
/** CSS box sizing applies identically to authored width, flex basis and width constraints. */
pub(super) fn content_size(node: &Node, value: f64, horizontal: bool) -> f64 {
    if node.style.box_sizing == BoxSizing::BorderBox {
        (value
            - if horizontal {
                horizontal_edges(node)
            } else {
                vertical_edges(node)
            })
        .max(0.0)
    } else {
        value
    }
}
/** An authored height (content or border box per box-sizing) as a border-box height. */
pub(super) fn border_height(node: &Node, value: f64) -> f64 {
    if node.style.box_sizing == BoxSizing::BorderBox {
        value.max(vertical_edges(node))
    } else {
        value + vertical_edges(node)
    }
}
/** The content height inside an authored border-box or content-box height of a wrapper style. */
pub(super) fn content_height(style: &crate::document::ComputedStyle, value: f64) -> f64 {
    let sides = border_sides(style);
    let edges = style.padding[0] + style.padding[2] + sides[0] + sides[2];
    if style.box_sizing == BoxSizing::BorderBox {
        (value - edges).max(0.0)
    } else {
        value
    }
}
/** Percent widths resolve against the definite containing content extent. */
pub(super) fn specified_content(node: &Node, available: f64) -> Option<f64> {
    node.style.width.map(|width| {
        content_size(
            node,
            if node.style.percent_width {
                available * width / 100.0
            } else {
                width
            },
            true,
        )
    })
}
/** The content-box max-width, a percentage resolved against `containing` (unconstrained without one). */
pub(super) fn max_width(node: &Node, containing: Option<f64>) -> f64 {
    let Some(value) = node.style.max_width else {
        return LAYOUT_EXTENT;
    };
    if !node.style.percent_max_width {
        return content_size(node, value, true);
    }
    containing.map_or(LAYOUT_EXTENT, |containing| {
        content_size(node, containing * value / 100.0, true)
    })
}
/** Auto blocks fill the available border box; specified sizes can overflow their containing block. */
pub(super) fn border_width(
    node: &Node,
    containing: f64,
    available: f64,
    natural: Option<f64>,
) -> f64 {
    let edges = horizontal_edges(node);
    let min = node
        .style
        .min_width
        .map(|width| content_size(node, width, true))
        .unwrap_or(0.0);
    let max = max_width(node, Some(containing)).max(min);
    let content = specified_content(node, containing).unwrap_or_else(|| {
        natural
            .unwrap_or((available - edges).max(0.0))
            .min((available - edges).max(0.0))
    });
    content.max(min).min(max) + edges
}
/** Native sizing preferences retain the established undecorated control dimensions. */
pub(super) fn text_intrinsic(
    context: &mut LayoutContext<'_>,
    node: &Node,
) -> EngineResult<IntrinsicWidths> {
    let face = face_for(&node.style, Some(&node.text));
    if node.tag == "svg" {
        let width = super::svg::intrinsic_width(node)?;
        return Ok(IntrinsicWidths {
            min_content: width,
            max_content: width,
        });
    }
    if node.tag == "img" {
        let dimensioned = node.style.height.is_some()
            || node.style.width.is_some()
            || node.attributes.contains_key("width");
        let width = if node.image_size.is_some() || dimensioned {
            let mut auto = node.clone();
            if auto.style.percent_width {
                auto.style.width = None;
                auto.style.percent_width = false;
            }
            (super::image::image_size(&auto, LAYOUT_EXTENT, None).0 - horizontal_edges(node))
                .max(0.0)
        } else {
            context.measure(&node.text, face)?
        };
        return Ok(IntrinsicWidths {
            min_content: width,
            max_content: width,
        });
    }
    if ["input", "button", "select", "textarea"].contains(&node.tag.as_str())
        && !super::control::has_content(context, node)
    {
        let (width, _) = super::control::natural_size(context, node)?;
        return Ok(IntrinsicWidths {
            min_content: width,
            max_content: width,
        });
    }
    context.charge(node.text.chars().count())?;
    let text = super::text::transformed(&node.style, &node.text);
    let words = text.split_whitespace().collect::<Vec<_>>();
    let mut minimum: f64 = 0.0;
    for word in &words {
        minimum = minimum.max(context.measure(word, face)?);
    }
    let maximum = if words.len() <= 1 {
        minimum
    } else {
        context.measure(&words.join(" "), face)?
    };
    if matches!(
        node.style.white_space,
        crate::document::WhiteSpace::Nowrap | crate::document::WhiteSpace::Pre
    ) {
        minimum = maximum;
    }
    Ok(IntrinsicWidths {
        min_content: minimum,
        max_content: maximum,
    })
}

/** Measures a container's own leading text as an anonymous flex item at `width`. */
pub(super) fn measure_own_text_height(
    context: &mut LayoutContext<'_>,
    node: &Node,
    width: f64,
) -> EngineResult<f64> {
    let mut target = flow::Flow::new(0.0, 0.0, width.max(0.0));
    target.strut = Some(flow::strut(&node.style));
    super::text::text(context, &node.text, node, &mut target, 0)?;
    target.newline();
    Ok(target.bottom.max(0.0))
}

/** Places a flex container's own leading text into its `DisplayItem` at `(x, y, width)`. */
pub(super) fn place_own_text(
    context: &mut LayoutContext<'_>,
    node: &Node,
    (x, y, width): (f64, f64, f64),
    container_index: usize,
    output: &mut [DisplayItem],
) -> EngineResult<()> {
    let mut target = flow::Flow::new(x, y, width.max(0.0));
    target.strut = Some(flow::strut(&node.style));
    target.align = flow::alignment(&node.style);
    output[container_index].lines =
        super::text::text(context, &node.text, node, &mut target, container_index)?;
    target.newline();
    super::line::settle(context, &mut target, output, output.len())
}

/** Intrinsic contributions clamp child constraints before adding decorations and margins. */
fn intrinsic_contribution(parent: &Node, child: &Node, raw: IntrinsicWidths) -> IntrinsicWidths {
    let preferred = (!child.style.percent_width)
        .then(|| specified_content(child, 0.0))
        .flatten();
    let minimum = child
        .style
        .min_width
        .map(|value| content_size(child, value, true))
        .unwrap_or(0.0);
    let maximum = max_width(child, None).max(minimum);
    let clamp = |value: f64| value.max(minimum).min(maximum);
    let mut contribution = IntrinsicWidths {
        min_content: clamp(preferred.unwrap_or(raw.min_content)),
        max_content: clamp(preferred.unwrap_or(raw.max_content)),
    };
    if parent.style.display_flex && parent.style.flex_direction == FlexDirection::Row {
        row_basis_contribution(parent, child, raw, preferred, &mut contribution);
    }
    contribution
}

/** Pinned Blink row intrinsic rules preserve definite bases that zero flex factors cannot move. */
fn row_basis_contribution(
    parent: &Node,
    child: &Node,
    raw: IntrinsicWidths,
    preferred: Option<f64>,
    contribution: &mut IntrinsicWidths,
) {
    let basis = match child.style.flex_basis {
        crate::document::FlexBasis::Pixels(value) => Some(content_size(child, value, true)),
        crate::document::FlexBasis::Auto => preferred,
        crate::document::FlexBasis::Percent(_) => None,
    };
    let Some(base) = basis else {
        return;
    };
    let maximum = max_width(child, None);
    let minimum = child
        .style
        .min_width
        .map(|value| content_size(child, value, true))
        .unwrap_or(
            raw.min_content
                .min(preferred.unwrap_or(LAYOUT_EXTENT))
                .min(maximum),
        );
    let hypothetical = base.max(minimum).min(maximum.max(minimum));
    let cant_move = |value: f64| {
        (value > base && child.style.flex_grow == 0.0)
            || (value < base && child.style.flex_shrink == 0.0)
    };
    if parent.style.flex_wrap == FlexWrap::NoWrap && cant_move(contribution.min_content) {
        contribution.min_content = hypothetical;
    }
    if cant_move(contribution.max_content) {
        contribution.max_content = hypothetical;
    }
    contribution.max_content = contribution.max_content.max(contribution.min_content);
}
/** A `display: contents` element with no text or generated boxes of its own, so it needs no box. */
fn contents(node: &Node) -> bool {
    node.style.display_contents
        && node.text.trim().is_empty()
        && node.style.generated.is_empty()
        && !node.hidden()
}
/** Some padding or margin of this style is a percentage of its containing block's width. */
pub(super) fn has_percent_sides(style: &crate::document::ComputedStyle) -> bool {
    style
        .padding_percent
        .iter()
        .chain(&style.margin_percent)
        .any(|percent| *percent != 0.0)
}
/**
 * A copy of the document whose percentage paddings and margins are pixels: each against the
 * content-box width its containing block had in `items`, the nearest laid-out block ancestor,
 * else the body's content box; the body's own against the viewport.
 * ponytail: absolute boxes resolve against their parent's content box, not their positioned
 * ancestor's padding box; resolve against the containing block layout picks if that shows up.
 */
pub(super) fn resolve_percent_sides(
    document: &Document,
    items: &[DisplayItem],
    viewport: f64,
) -> Document {
    let mut resolved = document.clone();
    sides_to_pixels(&mut resolved.formatting.body.style, viewport);
    let root = root_content_width(&resolved, viewport);
    let mut first = BTreeMap::new();
    for item in items {
        first.entry(item.local_id).or_insert(item.width);
    }
    let positions = positions(document);
    let mut content: Vec<Option<f64>> = vec![None; resolved.nodes.len()];
    for index in 0..resolved.nodes.len() {
        let containing = containing_width(&resolved, &positions, &content, index).unwrap_or(root);
        let node = &mut resolved.nodes[index];
        sides_to_pixels(&mut node.style, containing);
        let block = !node.style.inline || node.style.inline_block;
        if let Some(width) = first.get(&node.local_id).filter(|_| block) {
            content[index] = Some((width - horizontal_edges(node)).max(0.0));
        }
        let own = content[index].unwrap_or(containing);
        for generated in &mut node.style.generated {
            sides_to_pixels(&mut generated.style, own);
        }
    }
    resolved
}
/** The nearest ancestor's measured content width, walking past boxes that had none. */
fn containing_width(
    document: &Document,
    positions: &[Option<usize>],
    content: &[Option<f64>],
    index: usize,
) -> Option<f64> {
    let mut current = document.nodes[index].parent;
    while let Some(local) = current {
        let position = positions.get(local).copied().flatten()?;
        if let Some(width) = content[position] {
            return Some(width);
        }
        current = document.nodes[position].parent;
    }
    None
}
/** The body's content-box width, which top-level boxes resolve their percentages against. */
fn root_content_width(document: &Document, viewport: f64) -> f64 {
    if document.formatting.body_box_authored {
        super::body::content(&document.formatting.body.style, viewport).2
    } else {
        (viewport - super::MARGIN * 2.0).min(super::CONTENT_WIDTH)
    }
}
/** Folds each percentage side into its pixels against `basis`, leaving no percentage behind. */
fn sides_to_pixels(style: &mut crate::document::ComputedStyle, basis: f64) {
    use crate::constants::CSS_MAX_SCALAR;
    for side in 0..4 {
        let padding = style.padding[side] + basis * style.padding_percent[side] / 100.0;
        style.padding[side] = padding.clamp(0.0, CSS_MAX_SCALAR);
        let margin = style.margin[side] + basis * style.margin_percent[side] / 100.0;
        style.margin[side] = margin.clamp(-CSS_MAX_SCALAR, CSS_MAX_SCALAR);
    }
    style.padding_percent = [0.0; 4];
    style.margin_percent = [0.0; 4];
}
