use super::validation::bounded;
use super::{DisplayItem, flow};
/** Shared sizing caches and work limits keep intrinsic, dry-run and final layout bounded. */
use crate::document::style::{MIN_WIDTH, WIDTH, calc_size_through};
use crate::{
    constants::{LAYOUT_EXTENT, LAYOUT_MEASURES, LAYOUT_WORK},
    document::{BoxSizing, Document, FlexDirection, FlexWrap, Node, style::SizeKeyword},
    platform::{Face, Family, measure},
    protocol::{EngineError, EngineResult},
};
use std::collections::BTreeMap;

mod block_keywords;
mod containment;
mod natural;
mod quirks;
mod replaced;
mod stretch;
mod transfer;
pub(super) use block_keywords::{clamp_content_bounds, has_content_bounds};
pub(super) use containment::contained_height;
pub(super) use quirks::quirk_definite;
pub(super) use replaced::{
    replaced_block_at, replaced_contribution, replaced_fit, replaced_ratio, replaced_size,
    replaced_suggestion,
};
pub(super) use stretch::{clamp_stretch, stretch_height, stretched};
pub(super) use transfer::{min_width_auto, transferred_width};

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
    wrapped: BTreeMap<(u64, FaceKey, bool), BTreeMap<String, Vec<super::wrap::MeasuredText>>>,
    /** Out-of-flow boxes by identity with their latest static position, placed after normal flow. */
    deferred: BTreeMap<usize, (f64, f64)>,
    /** Static elements whose absolute or fixed `::before`/`::after` need an ancestor's box, with
    the element's padding-box origin as the static position; placed with the other out-of-flow boxes. */
    deferred_generated: BTreeMap<usize, (f64, f64)>,
    /** Each laid-out grid's track geometry, which its absolutely positioned descendants take
    their grid area from once its own box is placed. */
    grid_frames: BTreeMap<usize, super::grid::abspos::Frame>,
    /** A bound ran out; the rest of the page is not laid out. */
    exhausted: bool,
    /** List item ordinals by identity, computed once per list. */
    ordinals: BTreeMap<usize, i64>,
    /** Each block's latest content-derived border-box height, before any authored height. */
    content_heights: BTreeMap<usize, f64>,
    /** Inline boxes' extent past their first and last runs (child margins, own spaces). */
    inline_edges: BTreeMap<usize, (f64, f64)>,
    /** The viewport (width, height): the initial containing block, whose height is the room
    an orthogonal (vertical) flow gets when its container's height is not definite. */
    pub viewport: (f64, f64),
    /** The body's padding box `[x, y, width, height]` when the body is positioned: the
    containing block of top-level absolute boxes instead of the viewport (CSS2 abspos-009). The
    body is formatting data, not a node, so layout records it before placing them. */
    pub body_block: Option<[f64; 4]>,
    /** The column flex item being placed whose flexed height is not definite (Blink's
    `IsInitialBlockSizeIndefinite`): its percentage-height children resolve as `auto`. */
    pub indefinite_item: Option<usize>,
    /** Out-of-flow boxes whose static position is their right edge (`mark_static_end`). */
    static_end: std::collections::BTreeSet<usize>,
    /** Out-of-flow boxes whose static position is their horizontal center (`mark_static_center`). */
    static_center: std::collections::BTreeSet<usize>,
    /** A `::first-letter` waiting for the first text of the block that set it: that block's
    identity and the letter's style (render/first_letter.rs). */
    pub first_letter: Option<(usize, Box<crate::document::ComputedStyle>)>,
    /** The block whose first formatted line is open, with its `::first-line` fonts
    (render/first_line_layout.rs). */
    pub first_line: Option<super::first_line_layout::Pending>,
    /** Nested orthogonal layouts temporarily own the thread's free-function keyword lookup. */
    _keyword_scope: KeywordScope,
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
        let mut context = Self {
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
            deferred_generated: BTreeMap::new(),
            grid_frames: BTreeMap::new(),
            exhausted: false,
            ordinals: BTreeMap::new(),
            content_heights: BTreeMap::new(),
            inline_edges: BTreeMap::new(),
            viewport: (0.0, 0.0),
            body_block: None,
            indefinite_item: None,
            static_end: std::collections::BTreeSet::new(),
            static_center: std::collections::BTreeSet::new(),
            first_letter: None,
            first_line: None,
            _keyword_scope: KeywordScope::new(),
        };
        context.measure_keyword_boxes();
        Ok(context)
    }
    /** Measures every box whose width, min-width or max-width is a content keyword, so the
     * free sizing functions can resolve the keyword against its intrinsic widths. A box past the
     * work bound keeps no entry, and its keyword then behaves as `auto`. */
    fn measure_keyword_boxes(&mut self) {
        let keyword = |node: &&Node| {
            node.style
                .size_keywords
                .iter()
                .any(|keyword| *keyword != SizeKeyword::None)
        };
        for node in self.document.nodes.iter().filter(keyword) {
            let _ = self.intrinsic_widths(node.local_id);
        }
    }
    /** Publish completed dependencies immediately, before an ancestor reads their keyword constraints. */
    fn remember_intrinsic(&mut self, node: &Node, widths: IntrinsicWidths) -> IntrinsicWidths {
        self.intrinsic.insert(node.local_id, widths);
        if node
            .style
            .size_keywords
            .iter()
            .any(|keyword| *keyword != SizeKeyword::None)
        {
            KEYWORD_WIDTHS.with(|cache| cache.borrow_mut().insert(node.local_id, widths));
        }
        widths
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
        breakable: bool,
    ) -> EngineResult<Option<Vec<super::wrap::MeasuredText>>> {
        let key = (width.to_bits(), face_key(face), breakable);
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
        breakable: bool,
        lines: &[super::wrap::MeasuredText],
    ) -> EngineResult<()> {
        self.charge(text.chars().count() * 2 + 1)?;
        let key = (width.to_bits(), face_key(face), breakable);
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
    /** Marks an out-of-flow box whose static position a right-to-left line recorded: its right
    edge, not its left, goes there. */
    pub fn mark_static_end(&mut self, local: usize) {
        self.static_end.insert(local);
    }
    /** Whether `mark_static_end` marked this box. */
    pub fn static_end(&self, local: usize) -> bool {
        self.static_end.contains(&local)
    }
    /** Marks an out-of-flow box whose horizontal static position is a center point (a flex
    container's centered sole item): its inset-modified containing block is centered there. */
    pub fn mark_static_center(&mut self, local: usize) {
        self.static_center.insert(local);
    }
    /** Whether `mark_static_center` marked this box. */
    pub fn static_center(&self, local: usize) -> bool {
        self.static_center.contains(&local)
    }
    /** Records a grid's latest track geometry; the final layout pass overwrites dry runs. */
    pub(super) fn record_grid_frame(&mut self, local: usize, frame: super::grid::abspos::Frame) {
        self.grid_frames.insert(local, frame);
    }
    /** A grid's recorded track geometry, if it was laid out as a grid. */
    pub(super) fn grid_frame(&self, local: usize) -> Option<&super::grid::abspos::Frame> {
        self.grid_frames.get(&local)
    }
    /** Records a static element whose positioned generated boxes wait for their containing block. */
    pub fn defer_generated(&mut self, local: usize, position: (f64, f64)) {
        self.deferred_generated.insert(local, position);
    }
    /** Takes every element recorded by `defer_generated`, in document order. */
    pub fn take_deferred_generated(&mut self) -> Vec<(usize, (f64, f64))> {
        let mut taken = std::mem::take(&mut self.deferred_generated)
            .into_iter()
            .collect::<Vec<_>>();
        taken.sort_by_key(|(local, _)| self.positions.get(*local).copied().flatten());
        taken
    }
    /** Records a block's content-derived border-box height from its latest layout. */
    pub fn record_content_height(&mut self, local: usize, height: f64) {
        self.content_heights.insert(local, height);
    }
    /** Records how far an inline box's content edges sit before its first run and after its last. */
    pub fn record_inline_edges(&mut self, local: usize, edges: (f64, f64)) {
        self.inline_edges.insert(local, edges);
    }
    /** An inline box's recorded content edges past its runs, zero when none were recorded. */
    pub fn inline_edges(&self, local: usize) -> (f64, f64) {
        self.inline_edges.get(&local).copied().unwrap_or_default()
    }
    /** A block's content-derived border-box height from its latest layout, if it was laid out. */
    pub fn content_height(&self, local: usize) -> Option<f64> {
        self.content_heights.get(&local).copied()
    }
    /** Moves the recorded static position of every out-of-flow box inside `root` down by `dy`,
     * for a layout that shifts `root`'s content after the fact (a button centering its content):
     * the boxes are placed later and would otherwise keep the unshifted position. */
    pub fn shift_deferred_within(&mut self, root: usize, dy: f64) -> EngineResult<()> {
        let locals = self.deferred.keys().copied().collect::<Vec<_>>();
        for local in locals {
            let mut parent = self.node(local)?.parent;
            for _ in 0..crate::constants::DOCUMENT_DEPTH {
                let Some(ancestor) = parent else { break };
                if ancestor == root {
                    if let Some(position) = self.deferred.get_mut(&local) {
                        position.1 += dy;
                    }
                    break;
                }
                parent = self.node(ancestor)?.parent;
            }
        }
        Ok(())
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
        children.retain(|node| !node.removed());
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
    /** Intrinsic measurement is memoized per stable node for the entire request. */
    pub fn intrinsic_widths(&mut self, local: usize) -> EngineResult<IntrinsicWidths> {
        if let Some(value) = self.intrinsic.get(&local) {
            return Ok(*value);
        }
        self.charge(1)?;
        let node = self.node(local)?;
        if let Some(size) = containment::contained_widths(node) {
            return Ok(self.remember_intrinsic(node, size));
        }
        if super::vertical::is_root(self, node) {
            let width = super::vertical::intrinsic_width(self, node)?;
            let content = (width - horizontal_edges(node)).max(0.0);
            let size = IntrinsicWidths {
                min_content: content,
                max_content: content,
            };
            return Ok(self.remember_intrinsic(node, size));
        }
        if super::table::is_table(self, node) {
            let size = super::table::intrinsic(self, node)?;
            return Ok(self.remember_intrinsic(node, size));
        }
        let mut size = text_intrinsic(self, node)?;
        let mut children = self.children(local);
        children.retain(|child| !flow::out_of_flow(child));
        let indent = super::indent_intrinsic::indent(node, &children);
        let own_first = size.min_content;
        let mut first_min = None;
        // The widest piece after the first one, which the indent does not widen.
        let mut rest_min: f64 = 0.0;
        let mut sum = if node.style.display_flex && !node.text.trim().is_empty() {
            size
        } else {
            IntrinsicWidths::default()
        };
        let mut maxes = Vec::with_capacity(children.len());
        for child in &children {
            let value = super::flex_layout::row_item_widths(self, node, child)?;
            let contribution = intrinsic_contribution(node, child, value);
            let extra = intrinsic_extra(child);
            let minimum = contribution.min_content;
            if first_min.is_some() {
                rest_min = rest_min.max(minimum + extra);
            }
            first_min.get_or_insert(minimum + extra);
            // A table caption is as wide as its table, so only its min-content widens it
            // (Wikipedia thumbnails were as wide as their one-line caption).
            let caption = node.style.table_box == crate::document::style::TableBox::Table
                && child.style.table_box == crate::document::style::TableBox::Caption;
            let max = if caption {
                minimum
            } else {
                contribution.max_content
            };
            size.min_content = size.min_content.max(minimum + extra);
            size.max_content = size.max_content.max(max + extra);
            sum.min_content += minimum + extra;
            sum.max_content += max + extra;
            maxes.push(max + extra);
        }
        let mut generated_max = 0.0;
        let generated = self.generated_widths(node)?;
        let generated_items = generated.len();
        for generated in generated {
            size.min_content = size.min_content.max(generated.min_content);
            size.max_content = size.max_content.max(generated.max_content);
            sum.min_content += generated.min_content;
            sum.max_content += generated.max_content;
            generated_max += generated.max_content;
        }
        if node.style.display_grid {
            size = self.grid_intrinsic(node, &children, size)?;
        } else if node.style.display_flex && node.style.flex_direction == FlexDirection::Row {
            // `::before` and `::after` are items too, so each one adds a gap.
            let items =
                children.len() + generated_items + usize::from(!node.text.trim().is_empty());
            let gaps = node.style.column_gap * items.saturating_sub(1) as f64;
            size.max_content = match super::flex::Breaking::of(node) {
                super::flex::Breaking::Balance(min) => size.max_content.max(
                    super::flex::balanced_max_content(&maxes, node.style.column_gap, min),
                ),
                _ => size.max_content.max(sum.max_content + gaps),
            };
            if node.style.flex_wrap == FlexWrap::NoWrap {
                size.min_content = size.min_content.max(sum.min_content + gaps);
            }
        } else if let Some(width) =
            super::flex_column_intrinsic::max_content(self, node, &children, size.max_content)?
        {
            size.max_content = width.max(size.min_content);
        } else if children
            .iter()
            .all(|child| flow::inline_level(child) || floating(child))
        {
            let line = self.longest_line(node, &children, &maxes, generated_max + indent)?;
            // An indent changes the first line, which `line` measured with every other line.
            size.max_content = if indent != 0.0 {
                line.max(0.0)
            } else {
                size.max_content.max(line)
            };
            let own = !node.text.trim().is_empty();
            let (first, rest) = match own {
                true => (Some(own_first), rest_min.max(first_min.unwrap_or(0.0))),
                false => (first_min, rest_min),
            };
            size = super::indent_intrinsic::first_line(node, &children, (first, rest), size);
        } else {
            let line = self.longest_run(node, &children, &maxes, generated_max)?;
            size.max_content = size.max_content.max(line);
        }
        if super::multicol::applies(node) {
            size = super::multicol_intrinsic::intrinsic(self, node, size)?;
        }
        let basis = node
            .style
            .aspect_ratio
            .and_then(|_| super::image::percent_basis(self, node));
        let size = transfer::ratio_widths(node, size, basis);
        bounded(size.min_content)?;
        bounded(size.max_content)?;
        Ok(self.remember_intrinsic(node, size))
    }
    /** In-flow `::before` and `::after` boxes are the element's first and last children in Blink's
     * layout tree, so their content counts toward its min- and max-content widths with their
     * own edges. GNU's floated "»" tab was only its padding wide, so one more tab fit beside it. */
    fn generated_widths(&mut self, node: &Node) -> EngineResult<Vec<IntrinsicWidths>> {
        let mut widths = Vec::new();
        for generated in super::generated::items(node).into_iter().flatten() {
            let raw = super::flex_layout::item_intrinsic(self, &generated)?;
            let value = intrinsic_contribution(node, &generated, raw);
            let extra = intrinsic_extra(&generated);
            widths.push(IntrinsicWidths {
                min_content: value.min_content + extra,
                max_content: value.max_content + extra,
            });
        }
        Ok(widths)
    }
    /** A grid's widths come from its column track sizing algorithm, which `grid` owns. */
    fn grid_intrinsic(
        &mut self,
        node: &Node,
        _children: &[&Node],
        _size: IntrinsicWidths,
    ) -> EngineResult<IntrinsicWidths> {
        super::grid::intrinsic(self, node)
    }
    /** Max-content of inline content split by blocks (CSS 2.1 9.2.1.1): each run of inline
     * children between blocks is a line of its own, so the widest run counts. Without it an
     * inline-table beside text took only its own width and wrapped the text around it
     * (inline-table-valign-001). */
    fn longest_run(
        &mut self,
        node: &Node,
        children: &[&Node],
        maxes: &[f64],
        generated: f64,
    ) -> EngineResult<f64> {
        let mut longest = 0.0_f64;
        let mut start = 0;
        for end in 0..=children.len() {
            let block = children
                .get(end)
                .is_some_and(|c| super::block_in_inline::is_block(c));
            if end < children.len() && !block {
                continue;
            }
            if end > start {
                let own = if start == 0 { generated } else { 0.0 };
                let run =
                    self.longest_line(node, &children[start..end], &maxes[start..end], own)?;
                longest = longest.max(run);
            }
            start = end + 1;
        }
        Ok(longest)
    }
    /** Max-content of inline children: a forced break (`<br>`) ends a line, so the widest
     * line counts, not their sum (Blink's ComputeMinMaxSizes breaks at forced breaks). A cell
     * holding "tall<br>cell<br>here" was as wide as all three words. */
    fn longest_line(
        &mut self,
        node: &Node,
        children: &[&Node],
        maxes: &[f64],
        generated: f64,
    ) -> EngineResult<f64> {
        super::forced_lines::longest(self, node, (children, maxes), generated)
    }
    /** What one line of inline children adds beyond their own widths: the element's own leading
     * text and the spaces between runs. Without them HN's inner table was too narrow by its
     * " (" and ")" and wrapped a title Chromium keeps on one line. */
    pub(super) fn inline_extras(
        &mut self,
        node: &Node,
        children: &[&Node],
        first: bool,
    ) -> EngineResult<f64> {
        let own = if first { node.text.trim() } else { "" };
        let mut extra = if own.is_empty() {
            0.0
        } else {
            text_intrinsic(self, node)?.max_content
        };
        let space = self.measure(" ", face_for(&node.style, Some(" ")))?;
        // Collapsible spaces between adjacent runs count once per boundary, whichever side
        // holds them (`<a>new</a> | <a>past</a>` has two).
        // Preserved white space is measured with its own text, so it adds no boundary space.
        let collapsible = |child: &Node| {
            !matches!(
                child.style.white_space,
                crate::document::WhiteSpace::Pre
                    | crate::document::WhiteSpace::PreWrap
                    | crate::document::WhiteSpace::PreLine
            )
        };
        let leading = |child: &Node| {
            collapsible(child)
                && (child.text.starts_with(char::is_whitespace) || child.space_leading)
        };
        let trailing = |child: &Node| {
            child.space_after
                || (collapsible(child)
                    && child.tag == "#text"
                    && child.text.ends_with(char::is_whitespace))
        };
        let mut boundaries = children
            .windows(2)
            // A space beside a float collapses: floats leave the line, so it starts or ends there.
            .filter(|pair| !floating(pair[0]) && !floating(pair[1]))
            .filter(|pair| trailing(pair[0]) || leading(pair[1]))
            .count();
        if let Some(first) = children.first().filter(|_| !own.is_empty()) {
            boundaries += usize::from(node.text.ends_with(char::is_whitespace) || leading(first));
        }
        extra += space * boundaries as f64;
        Ok(extra)
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
type FaceKey = (u64, u16, Family, u64, bool, u8);
/** Floats key by their bits so identical faces share cached shaping. */
fn face_key(face: Face) -> FaceKey {
    (
        face.size.to_bits(),
        face.weight,
        face.family,
        face.spacing.to_bits(),
        face.italic,
        face.autospace,
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
        autospace: crate::platform::autospace::mode(
            style.text_autospace && style.glyphs_upright != crate::platform::upright::UPRIGHT,
            &style.lang,
        ),
        upright: style.glyphs_upright,
    }
}
/// Distinct faces whose native metrics are cached; the cache resets rather than grow past this.
const ADVANCE_CACHE: usize = 256;
thread_local! {
    /** Native ascender and normal line height per face; asking AppKit for every word would dominate layout. */
    static METRICS: std::cell::RefCell<BTreeMap<FaceKey, (f64, f64)>> = const { std::cell::RefCell::new(BTreeMap::new()) };
}
/** The face's ascender and `line-height: normal`, finite and positive. */
pub(super) fn face_metrics(style: &crate::document::ComputedStyle) -> (f64, f64) {
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
        let (ascent, normal) = blink_metrics(face);
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
/// Device pixels per CSS px Chromium lays text out at on the Retina Macs Mars targets; Blink
/// computes font metrics at the device font size (zoom-for-DSF), so rounding happens there.
// ponytail: fixed 2x; a 1x display rounds at whole CSS px, pass the window's scale when one exists.
pub(super) const DEVICE_SCALE: f64 = 2.0;
/// Safari's and Blink's macOS ascent hack for Times and Helvetica: 15% of the rounded height.
const CLASSIC_ASCENT: f64 = 0.15;
/**
 * Ascender and `line-height: normal` as Blink's FontMetrics::AscentDescentWithHacks and
 * SimpleFontData::PlatformInit compute them: at the device font size, ascent and descent are
 * each rounded to whole device pixels, Times and Helvetica gain `floor(15% of their height +
 * 0.5)` on the ascent, and line spacing is the sum of the rounded parts. Unrounded metrics left
 * every Helvetica line 0.1 px short of Chromium's 18.5, so long lists drifted.
 */
fn blink_metrics(face: crate::platform::Face) -> (f64, f64) {
    let device = crate::platform::Face {
        size: face.size * DEVICE_SCALE,
        ..face
    };
    let (ascent, normal) = crate::platform::metrics(device);
    let classic = matches!(
        face.family,
        crate::platform::Family::Serif | crate::platform::Family::SansSerif
    );
    // The platform folds the hack into its ascent unrounded; take it back out first.
    let below = normal - ascent;
    let ascent = if classic {
        (ascent - CLASSIC_ASCENT * below) / (1.0 + CLASSIC_ASCENT)
    } else {
        ascent
    };
    let (ascent, below) = (ascent.round(), below.round());
    let hack = if classic {
        ((ascent + below) * CLASSIC_ASCENT + 0.5).floor()
    } else {
        0.0
    };
    (
        (ascent + hack) / DEVICE_SCALE,
        (ascent + hack + below) / DEVICE_SCALE,
    )
}
/** The used line advance: `normal` is the face's own line height, as Chromium computes it. */
pub(super) fn advance(style: &crate::document::ComputedStyle) -> f64 {
    if style.line_height != crate::document::LineHeight::Normal {
        return style.line_height.used(style.font_size);
    }
    face_metrics(style).1
}
/** Distance from a line's top to its text baseline: the leading above plus the ascender. */
pub(super) fn text_ascent(style: &crate::document::ComputedStyle) -> f64 {
    face_metrics(style).0 + half_leading(style)
}
/**
 * Blink's CalculateLeadingSpace: the leading above the content area is half of line-height
 * minus the font height, floored to a whole layout pixel (a device pixel at 2x), and negative
 * when line-height is smaller than the font. Clamping it at zero made a `line-height: 30px` line
 * holding 30px text 2.25px taller than Chromium's.
 */
pub(super) fn half_leading(style: &crate::document::ComputedStyle) -> f64 {
    let (_, normal) = face_metrics(style);
    ((advance(style) - normal) / 2.0 * DEVICE_SCALE).floor() / DEVICE_SCALE
}
/** How far `vertical-align: <length-percentage>` raises this box's baseline; a percentage is
 * of its own line-height (CSS 2.1 10.8.1, Blink's kLength case). */
pub(super) fn baseline_shift(style: &crate::document::ComputedStyle) -> f64 {
    style
        .vertical_shift
        .map_or(0.0, |(px, percent)| px + percent / 100.0 * advance(style))
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
    let sides = if style.border > 0.0 {
        [style.border; 4]
    } else {
        style.border_widths
    };
    sides.map(snap_border)
}
/** Blink's ConvertBorderWidth at the device scale: a border is a whole number of device
 * pixels, rounded down, and at least one when it is not zero (`.1em` is 1.5px at 2x). */
pub(super) fn snap_border(width: f64) -> f64 {
    if width <= 0.0 {
        return 0.0;
    }
    (width * DEVICE_SCALE).floor().max(1.0) / DEVICE_SCALE
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
/** Move, rather than clone, the enclosing layout's cache while an orthogonal layout runs. */
struct KeywordScope {
    previous: BTreeMap<usize, IntrinsicWidths>,
    /** This guard must restore on its creating thread, like the lookup it owns. */
    _thread: std::marker::PhantomData<std::rc::Rc<()>>,
}
impl KeywordScope {
    /** Begin with no identities from an earlier document, including during intrinsic premeasurement. */
    fn new() -> Self {
        Self {
            previous: KEYWORD_WIDTHS.with(|cache| std::mem::take(&mut *cache.borrow_mut())),
            _thread: std::marker::PhantomData,
        }
    }
}
impl Drop for KeywordScope {
    /** Error returns and successful nested layouts both restore the enclosing lookup. */
    fn drop(&mut self) {
        KEYWORD_WIDTHS.with(|cache| *cache.borrow_mut() = std::mem::take(&mut self.previous));
    }
}
thread_local! {
    /** Free sizing helpers read only their active layout's completed keyword-sized boxes. */
    static KEYWORD_WIDTHS: std::cell::RefCell<BTreeMap<usize, IntrinsicWidths>> =
        const { std::cell::RefCell::new(BTreeMap::new()) };
}
/** Which property a content keyword sizes, for its indefinite-percentage fallback. */
#[derive(Clone, Copy, PartialEq)]
enum Bound {
    Preferred,
    Min,
    Max,
}
/**
 * The content width a sizing keyword gives `node` (Blink's ResolveInlineLengthInternal for
 * kMinContent, kMaxContent, kFitContent and the fit-content() limit): min- or max-content, or a
 * limit clamped between them. `containing` is None while measuring contributions, where a
 * percentage limit is cyclic: it behaves as the property's initial value (`auto`, 0 or `none`).
 * None means the keyword leaves the size to the caller's ordinary path.
 */
fn keyword_width(
    node: &Node,
    keyword: SizeKeyword,
    containing: Option<f64>,
    bound: Bound,
    widths: IntrinsicWidths,
) -> Option<f64> {
    let fit = |limit: f64| limit.min(widths.max_content).max(widths.min_content);
    // The stretch-fit size of an indefinite container: `auto` as a preferred size (the caller's
    // own path), 0 as a minimum and unbounded as a maximum.
    let indefinite = match bound {
        Bound::Preferred => None,
        Bound::Min => Some(0.0),
        Bound::Max => Some(LAYOUT_EXTENT),
    };
    Some(match keyword {
        SizeKeyword::None => return None,
        SizeKeyword::MinContent => widths.min_content,
        SizeKeyword::MaxContent => widths.max_content,
        SizeKeyword::FitContent if bound == Bound::Preferred => return None,
        SizeKeyword::FitContent => fit(containing
            .map(|c| stretch_content(node, c))
            .or(indefinite)?),
        SizeKeyword::FitContentLimit(limit) => fit(match containing {
            Some(c) => content_size(
                node,
                limit.px + layout_unit_floor(c * limit.percent / 100.0),
                true,
            ),
            None if limit.percent != 0.0 => {
                return keyword_width(node, SizeKeyword::FitContent, None, bound, widths);
            }
            None => content_size(node, limit.px, true),
        }),
    })
}
/** `keyword_width` for one of the node's own properties, with the widths measured for it. */
fn own_keyword(node: &Node, bound: Bound, containing: Option<f64>) -> Option<f64> {
    let keyword = node.style.size_keywords[bound as usize];
    // On the block axis (a vertical writing mode) a content keyword behaves as the initial value.
    if keyword == SizeKeyword::None || node.style.vertical_writing {
        return None;
    }
    let widths = KEYWORD_WIDTHS.with(|cache| cache.borrow().get(&node.local_id).copied())?;
    keyword_width(node, keyword, containing, bound, widths)
}
/** Percent widths resolve against the definite containing content extent; a content keyword
 * resolves against the box's own intrinsic widths. */
pub(super) fn specified_content(node: &Node, available: f64) -> Option<f64> {
    if let Some(width) = own_keyword(node, Bound::Preferred, Some(available)) {
        return Some(width);
    }
    if node.style.width_stretch {
        return Some(stretch_content(node, available));
    }
    node.style.width.map(|width| {
        content_size(
            node,
            if node.style.percent_width {
                layout_unit_floor(available * width / 100.0) + node.style.width_offset
            } else {
                width
            },
            true,
        )
    })
}
/** `width: stretch`: the content width left once margins (auto ones as zero), border and padding
 * take their share of the containing block (Blink's `FillAvailableMeasure`). */
fn stretch_content(node: &Node, available: f64) -> f64 {
    let style = &node.style;
    let margin = |side: usize| {
        style.margin[side] + layout_unit_floor(available * style.margin_percent[side] / 100.0)
    };
    (available - margin(1) - margin(3) - horizontal_edges(node)).max(0.0)
}
/** Sub-pixel steps per CSS px in Chromium's fixed-point LayoutUnit. */
const LAYOUT_UNITS: f64 = 64.0;
/** A resolved percentage truncated to Chromium's 1/64 px LayoutUnit, so six `16.6666667%` floats
 * fit in their row instead of the last one wrapping by a ten-thousandth of a pixel (python.org's
 * top navigation). */
pub(super) fn layout_unit_floor(length: f64) -> f64 {
    (length * LAYOUT_UNITS).floor() / LAYOUT_UNITS
}
/** A text width rounded up to Chromium's 1/64 px LayoutUnit. */
pub(super) fn layout_unit_ceil(length: f64) -> f64 {
    (length * LAYOUT_UNITS).ceil() / LAYOUT_UNITS
}
/** The authored content-box min-width, a percentage resolved against `containing`; None when it
 * is `auto`, or a percentage with no containing width (intrinsic sizing treats that as zero). */
pub(super) fn min_width(node: &Node, containing: Option<f64>) -> Option<f64> {
    if let Some(width) = own_keyword(node, Bound::Min, containing) {
        return Some(width);
    }
    if let Some(percent) = node.style.min_width_percent {
        return containing.map(|containing| {
            content_size(node, layout_unit_floor(containing * percent / 100.0), true)
        });
    }
    node.style
        .min_width
        .map(|width| content_size(node, width, true))
}
/** The content-box max-width, a percentage resolved against `containing` (unconstrained without one). */
pub(super) fn max_width(node: &Node, containing: Option<f64>) -> f64 {
    if let Some(width) = own_keyword(node, Bound::Max, containing) {
        return width;
    }
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
    let min = min_width(node, Some(containing)).unwrap_or(0.0);
    let max = max_width(node, Some(containing)).max(min);
    // `width: stretch` fills the room the caller already took the margins out of, which beside
    // floats is the layout opportunity rather than the whole containing block.
    let stretch = node
        .style
        .width_stretch
        .then(|| (available - edges).max(0.0));
    let content = stretch
        .or_else(|| specified_content(node, containing))
        .unwrap_or_else(|| {
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
        let (min_content, max_content) = super::svg::intrinsic_width(node)?;
        return Ok(IntrinsicWidths {
            min_content,
            max_content,
        });
    }
    if super::flow::replaced_image(node) {
        return super::image::intrinsic(context, node);
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
    let own = super::text::own_text(node);
    let text = super::text::transformed(&node.style, &own);
    // CSS white space only: a no-break or ideographic space is part of its word, and takes
    // word-spacing as layout gives it.
    let words = text
        .split([' ', '\t', '\n', '\r', '\x0c'])
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();
    let mut minimum: f64 = 0.0;
    let mut sum = 0.0;
    let mut hang = 0.0;
    let breaks = super::text::min_content_breaks(context, node);
    for word in &words {
        let width =
            context.measure(word, face)? + super::text::spaces(word) * node.style.word_spacing;
        // Ideographic spaces ending a word hang at the end of the line it ends.
        hang = super::text::hanging(context, word, face)?;
        minimum = minimum.max(widest_piece(context, node, (word, breaks), face)? - hang);
        sum += width;
    }
    sum -= hang;
    // Max-content is the words and spaces line layout places, not the whole run shaped at once:
    // kerning across a space makes the run narrower, so a box sized to it wrapped its own text
    // (Google's "AI Mode" chip broke onto two lines).
    // One word is as wide as all of it, however finely it may break.
    let mut maximum = if words.len() <= 1 {
        sum.max(minimum)
    } else {
        let gap = context.measure(" ", face)? + node.style.word_spacing;
        sum + gap * (words.len() - 1) as f64
    };
    // Preserved spaces and forced breaks count as line layout places them: max-content is the
    // widest preserved line, spaces included (`pre-wrap`'s trailing spaces hang at a wrap but
    // Blink still counts them, so `<span>Day: </span><a>` keeps its gap as a flex item, and a
    // `pre` table cell holding one space is a space wide).
    if let Some(widest) = preserved_max(context, node, &text, face)? {
        maximum = widest;
    }
    if matches!(
        node.style.white_space,
        crate::document::WhiteSpace::Nowrap | crate::document::WhiteSpace::Pre
    ) {
        minimum = maximum;
    }
    // Blink keeps inline min/max-content sizes in LayoutUnits rounded up (FromFloatCeil), so a box
    // sized to its text never comes out a fraction narrower than the text it must hold.
    Ok(IntrinsicWidths {
        min_content: layout_unit_ceil(minimum),
        max_content: layout_unit_ceil(maximum),
    })
}

/** The widest piece of `word` its break opportunities leave unbreakable (a CJK character, a
phrase, the text between soft hyphens with its hyphen), which is the word's min-content. */
fn widest_piece(
    context: &mut LayoutContext<'_>,
    node: &Node,
    (word, breaks): (&str, super::wrap::Breaks),
    face: crate::platform::Face,
) -> EngineResult<f64> {
    let mut widest: f64 = 0.0;
    for piece in super::hyphenate::pieces(node, word, breaks) {
        let spacing = super::text::spaces(&piece) * node.style.word_spacing;
        widest = widest.max(context.measure(&piece, face)? + spacing);
    }
    Ok(widest)
}
/** The widest line of text whose white space keeps its spaces or its newlines, None for
collapsing white space, which the caller measures. A preserved line is one run, shaped whole
as line layout shapes it; a `pre-line` line is its words and one space between each. */
fn preserved_max(
    context: &mut LayoutContext<'_>,
    node: &Node,
    text: &str,
    face: crate::platform::Face,
) -> EngineResult<Option<f64>> {
    use crate::document::WhiteSpace;
    let style = &node.style;
    let keeps_spaces = matches!(style.white_space, WhiteSpace::Pre | WhiteSpace::PreWrap);
    if !keeps_spaces && style.white_space != WhiteSpace::PreLine {
        return Ok(None);
    }
    let mut widest: f64 = 0.0;
    for line in text.split('\n') {
        widest = widest.max(preserved_line(context, node, line, face)?);
    }
    Ok(Some(widest))
}
/** One line of `node`'s text between preserved newlines, as line layout places it: whole with
its tab stops where spaces are kept, its words and one space between each under `pre-line`. */
pub(super) fn preserved_line(
    context: &mut LayoutContext<'_>,
    node: &Node,
    line: &str,
    face: crate::platform::Face,
) -> EngineResult<f64> {
    use crate::document::WhiteSpace;
    let style = &node.style;
    if matches!(style.white_space, WhiteSpace::Pre | WhiteSpace::PreWrap) {
        // Tabs advance to CSS tab stops, which CoreText would not place.
        let mut width = 0.0;
        for (at, segment) in line.split('\t').enumerate() {
            if at > 0 {
                width += super::text::tab_width(context, node, width)?;
            }
            width +=
                context.measure(segment, face)? + super::text::spaces(segment) * style.word_spacing;
        }
        return Ok(width);
    }
    // CSS white space is space and tab only: a no-break space is a glyph.
    let words: Vec<&str> = line.split([' ', '\t']).filter(|w| !w.is_empty()).collect();
    let joined = words.join(" ");
    Ok(context.measure(&joined, face)? + super::text::spaces(&joined) * style.word_spacing)
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
    target.ellipsis = flow::ellipsis_owner(node);
    output[container_index].lines =
        super::text::text(context, &node.text, node, &mut target, container_index)?;
    target.newline();
    super::line::settle(context, &mut target, output, output.len())
}

/** Intrinsic contributions clamp child constraints before adding decorations and margins. */
pub(super) fn intrinsic_contribution(
    parent: &Node,
    child: &Node,
    raw: IntrinsicWidths,
) -> IntrinsicWidths {
    let [width, min, max] = if child.style.vertical_writing {
        [SizeKeyword::None; 3]
    } else {
        child.style.size_keywords
    };
    let preferred = if width == SizeKeyword::None {
        (!child.style.percent_width)
            .then(|| specified_content(child, 0.0))
            .flatten()
            .or_else(|| ratio_contribution(child, raw))
    } else {
        keyword_width(child, width, None, Bound::Preferred, raw)
    };
    // Blink's FitContentMode::kMinContribution and kMaxContribution: with no available width a
    // `fit-content` size is the min-content width in the min contribution and the max-content
    // width in the max one, whichever property names it.
    let side = |fit: f64| {
        let keyword = |keyword, bound| match keyword {
            SizeKeyword::FitContent => Some(fit),
            _ => keyword_width(child, keyword, None, bound, raw),
        };
        let preferred = if width == SizeKeyword::None {
            preferred
        } else {
            keyword(width, Bound::Preferred)
        };
        let minimum = keyword(min, Bound::Min)
            .or_else(|| {
                child
                    .style
                    .min_width
                    .map(|value| content_size(child, value, true))
            })
            .unwrap_or(0.0);
        let maximum = keyword(max, Bound::Max)
            .unwrap_or_else(|| max_width(child, None))
            .max(minimum);
        // A `calc-size()` width maps the size its basis gives (CSS Values 5).
        let preferred = calc_size_through(&child.style, WIDTH, preferred.unwrap_or(fit));
        preferred.max(minimum).min(maximum)
    };
    let mut contribution = IntrinsicWidths {
        min_content: side(raw.min_content),
        max_content: side(raw.max_content),
    };
    if parent.style.display_flex && parent.style.flex_direction == FlexDirection::Row {
        row_basis_contribution(parent, child, raw, preferred, &mut contribution);
    }
    contribution
}

/** Whether a child floats: floats sit side by side at max-content like inline content (Blink's
 * `floats_inline_size`), so two 50px float divs make a 100px box, not a 50px one. */
pub(super) fn floating(child: &Node) -> bool {
    child.style.float != crate::document::style::Float::None
}

/**
 * The content width an automatic-width box's `aspect-ratio` transfers from its definite height,
 * as Blink's ComputeMinAndMaxContentContribution does through InlineSizeFromAspectRatio. The
 * height is clamped by min- and max-height first, the ratio sizes the border box under
 * `box-sizing: border-box`, and min-width `auto` keeps it from going below the min-content
 * width (css-sizing-4's automatic minimum for ratio boxes). None without a ratio or with a
 * height that is `auto` or a percentage of the indefinite height being measured in.
 */
fn ratio_contribution(node: &Node, raw: IntrinsicWidths) -> Option<f64> {
    let ratio = node.style.aspect_ratio?;
    let height = node.style.height?.resolve_size(None)?;
    let bound = |extent: Option<crate::document::style::Extent>| {
        extent.and_then(|extent| extent.resolve_size(None))
    };
    let mut height = border_height(node, height);
    if let Some(max) = bound(node.style.max_height) {
        height = height.min(border_height(node, max));
    }
    if let Some(min) = bound(node.style.min_height) {
        height = height.max(border_height(node, min));
    }
    let width = if flow::ratio_sizes_border_box(node) {
        (height * ratio - horizontal_edges(node)).max(0.0)
    } else {
        (height - vertical_edges(node)).max(0.0) * ratio
    };
    let automatic = min_width_auto(&node.style);
    let width = calc_size_through(&node.style, WIDTH, width);
    Some(if automatic {
        width.max(calc_size_through(&node.style, MIN_WIDTH, raw.min_content))
    } else {
        width
    })
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
        crate::document::FlexBasis::Percent(_)
        | crate::document::FlexBasis::Mixed { .. }
        | crate::document::FlexBasis::Content => None,
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
        && !node.removed()
}
/** What a child's horizontal padding, border and margins add to its parent's min- and
 * max-content widths: only their fixed parts, as a percentage of the width being measured is
 * cyclic and Blink resolves it against zero there (`margin-left: calc(10% + 100px)` adds 100px). */
fn intrinsic_extra(child: &Node) -> f64 {
    let style = &child.style;
    let [pad_right, pad_left, margin_right, margin_left] = style.fixed_inline_sides.unwrap_or([
        style.padding[1],
        style.padding[3],
        style.margin[1],
        style.margin[3],
    ]);
    let sides = border_sides(style);
    pad_right + pad_left + sides[1] + sides[3] + margin_right + margin_left
}
/** Some padding or margin of this style is a percentage of its containing block's width. */
pub(super) fn has_percent_sides(style: &crate::document::ComputedStyle) -> bool {
    style
        .padding_percent
        .iter()
        .chain(&style.margin_percent)
        .any(|percent| *percent != 0.0)
}
/** Resolve the root against the viewport and the body against the root before measuring descendants. */
pub(super) fn resolve_wrapper_sides(document: &Document, viewport: f64) -> Document {
    let mut resolved = document.clone();
    sides_to_pixels(&mut resolved.formatting.html.style, viewport);
    let containing = super::root_box::root_box(&resolved, (viewport, 0.0)).content_width;
    sides_to_pixels(&mut resolved.formatting.body.style, containing);
    resolved
}
/**
 * A copy of the document whose percentage paddings and margins are pixels: each against the
 * content-box width its containing block had in `items`, the nearest laid-out block ancestor,
 * else the body's content box; wrapper edges resolve before descendant measurement.
 * ponytail: absolute boxes resolve against their parent's content box, not their positioned
 * ancestor's padding box; resolve against the containing block layout picks if that shows up.
 */
pub(super) fn resolve_percent_sides(
    document: &Document,
    items: &[DisplayItem],
    viewport: f64,
) -> Document {
    let mut resolved = resolve_wrapper_sides(document, viewport);
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
    // Only the horizontal edges are read, and they never depend on the viewport height.
    let root = super::body::root_edges(document, (viewport, 0.0));
    let inside = (viewport - root[1] - root[3]).max(0.0);
    if document.formatting.body_box_authored {
        super::body::content(&document.formatting.body.style, inside).2
    } else {
        (inside - super::MARGIN * 2.0).min(super::CONTENT_WIDTH)
    }
}
/** Folds each percentage side into its pixels against `basis`, leaving no percentage behind. */
fn sides_to_pixels(style: &mut crate::document::ComputedStyle, basis: f64) {
    use crate::constants::CSS_MAX_SCALAR;
    if has_percent_sides(style) {
        style.fixed_inline_sides = Some([
            style.padding[1],
            style.padding[3],
            style.margin[1],
            style.margin[3],
        ]);
    }
    for side in 0..4 {
        let padding = style.padding[side] + basis * style.padding_percent[side] / 100.0;
        style.padding[side] = padding.clamp(0.0, CSS_MAX_SCALAR);
        let margin = style.margin[side] + basis * style.margin_percent[side] / 100.0;
        style.margin[side] = margin.clamp(-CSS_MAX_SCALAR, CSS_MAX_SCALAR);
    }
    style.padding_percent = [0.0; 4];
    style.margin_percent = [0.0; 4];
}
