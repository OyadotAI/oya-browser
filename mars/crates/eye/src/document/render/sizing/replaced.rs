/** Replaced-element sizing, a port of Blink's ComputeReplacedSize (layout/length_utils.cc).
 *
 * Follows CSS 2.1 10.3.2, 10.4, 10.6.2 and 10.7 with css-sizing-4's aspect-ratio transfers:
 * natural sizes and the aspect ratio are normalised to the border box, each axis resolves its
 * main size and min/max constraints, an automatic axis takes its size from the other one through
 * the ratio, and min/max constraints on one axis transfer through the ratio to the other. All
 * sizes here are border-box sizes, as in Blink. */
use super::{content_size, horizontal_edges, layout_unit_floor, vertical_edges};
use crate::document::{
    BoxSizing, Node,
    style::{Extent, SizeKeyword},
};

/** No upper bound, Blink's `LayoutUnit::Max()`. */
const UNBOUNDED: f64 = f64::INFINITY;
/** The default natural size of a replaced box with neither natural size nor ratio (CSS 2.1 10.3.2). */
pub(super) const DEFAULT_NATURAL: (f64, f64) = (300.0, 150.0);

/** Which lengths a recursive pass ignores, Blink's `ReplacedSizeMode`. */
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    /** Resolve every length. */
    Normal,
    /** Ignore height, min-height and max-height: the size an automatic height derives from. */
    IgnoreBlock,
    /** Ignore width, min-width and max-width: the size an automatic width derives from. */
    IgnoreInline,
}

/** The constraint space one replaced box is sized in. */
struct Replaced<'a> {
    node: &'a Node,
    /** The containing block's content width; None while measuring intrinsic contributions. */
    available: Option<f64>,
    /** The definite containing block height percentages resolve against. */
    block_basis: Option<f64>,
    /** Horizontal and vertical border plus padding. */
    edges: (f64, f64),
    /** Width over height, and whether it applies to the border box rather than the content box. */
    ratio: Option<(f64, bool)>,
    /** Border-box natural size, coerced to the ratio when there is one. */
    natural: Option<(f64, f64)>,
    /** A border-box width a flex, grid or table algorithm already settled, which replaces the
    authored one and its min and max. */
    fixed: Option<f64>,
}

/** Clamps `size` into [min, max], min winning, as `MinMaxSizes::ClampSizeToMinAndMax`. */
fn clamp(size: f64, min: f64, max: f64) -> f64 {
    size.min(max).max(min)
}

/**
 * The border-box size of the replaced `node` with content width `available` and definite
 * containing height `block_basis`. Without an available width (intrinsic sizing) percentage
 * widths behave as `auto`, as Blink's indefinite inline size does.
 */
pub(in crate::document::render) fn replaced_size(
    node: &Node,
    available: Option<f64>,
    block_basis: Option<f64>,
) -> (f64, f64) {
    Replaced::new(node, available, block_basis).size(Mode::Normal)
}

/** The content width an inline `<svg>` takes laid out in `available` border-box width, or None
 * for any other box: a ratio-only svg stretches into a definite space (Blink's StretchFit) though
 * its contributions are zero. */
pub(in crate::document::render) fn replaced_fit(node: &Node, available: f64) -> Option<f64> {
    (node.tag == "svg").then(|| {
        let edges = horizontal_edges(node);
        let margins = node.style.margin[1] + node.style.margin[3];
        (replaced_size(node, Some(available + margins), None).0 - edges).max(0.0)
    })
}

/** The border-box height of the replaced `node` once something else settled its border-box
 * width at `inline`: Blink's ComputeReplacedSize with a fixed inline size. */
pub(in crate::document::render) fn replaced_block_at(
    node: &Node,
    inline: f64,
    block_basis: Option<f64>,
) -> f64 {
    let mut space = Replaced::new(node, Some(inline), block_basis);
    space.fixed = Some(inline);
    space.size(Mode::Normal).1
}

/**
 * Min- and max-content contributions of a replaced box, content box only, after Blink's
 * ComputeMinAndMaxContentContributionForReplaced: the max is its size at an indefinite width, and
 * a percentage width or max-width makes it compressible so its min shrinks to its min-width (an
 * `max-width: 100%` image no longer props its grid track or table column open).
 */
pub(in crate::document::render) fn replaced_contribution(
    node: &Node,
    block_basis: Option<f64>,
) -> (f64, f64) {
    let edges = horizontal_edges(node);
    let max = (replaced_size(node, None, block_basis).0 - edges).max(0.0);
    let compressible = node.style.percent_width || node.style.percent_max_width;
    let min = if compressible {
        let space = Replaced::new(node, None, block_basis);
        (space.min_inline(Mode::Normal, None) - edges)
            .max(0.0)
            .min(max)
    } else {
        max
    };
    (min, max)
}

/**
 * The content-box width a replaced box takes with its width properties ignored: its height
 * through the ratio, or its natural width. It is a flex item's content size suggestion
 * (css-flexbox-1 4.5), which an authored width never raises: a `width: 100px; height: 30px`
 * square image may shrink to 30px.
 */
pub(in crate::document::render) fn replaced_suggestion(
    node: &Node,
    block_basis: Option<f64>,
) -> f64 {
    let size = Replaced::new(node, None, block_basis).size(Mode::IgnoreInline);
    (size.0 - horizontal_edges(node)).max(0.0)
}

/** A replaced box's used aspect ratio and whether it sizes the border box, as `ratio` resolves
it from its natural size; a stretched flex item transfers its cross size through it. */
pub(in crate::document::render) fn replaced_ratio(node: &Node) -> Option<(f64, bool)> {
    Replaced::new(node, None, None).ratio
}

impl<'a> Replaced<'a> {
    /** Normalises the ratio and natural size as Blink's ComputeNormalizedNaturalSize does. */
    fn new(node: &'a Node, available: Option<f64>, block_basis: Option<f64>) -> Self {
        let edges = (horizontal_edges(node), vertical_edges(node));
        let natural = super::natural::natural(node);
        let mut space = Self {
            node,
            available,
            block_basis,
            edges,
            ratio: ratio(node, natural.ratio),
            natural: None,
            fixed: None,
        };
        space.natural = space.normalized(natural.width, natural.height);
        space
    }
    /** The border-box natural size: a missing dimension follows the ratio, or without one
    takes CSS 2.1's 300 by 150 default; the block size is coerced to the ratio when there is
    one. None when only a ratio is known. */
    fn normalized(&self, width: Option<f64>, height: Option<f64>) -> Option<(f64, f64)> {
        if width.is_none() && height.is_none() {
            return self
                .ratio
                .is_none()
                .then(|| unloaded(self.node, self.edges));
        }
        let unratioed = self.ratio.is_none();
        let inline = width
            .map(|w| w + self.edges.0)
            .or_else(|| unratioed.then_some(DEFAULT_NATURAL.0 + self.edges.0));
        let block = height
            .map(|h| h + self.edges.1)
            .or_else(|| unratioed.then_some(DEFAULT_NATURAL.1 + self.edges.1));
        let inline = inline.or_else(|| Some(self.inline_from_ratio(block?)))?;
        let block = match self.ratio {
            Some(_) => self.block_from_ratio(inline),
            None => block?,
        };
        Some((inline, block))
    }
    /** Blink's ComputeReplacedSizeInternal for one mode. */
    fn size(&self, mode: Mode) -> (f64, f64) {
        let (block_min, block_max, block) = self.block_constraints(mode);
        let (transfer_min, transfer_max) = self.transferred(block_min, block_max);
        let (inline_min, inline_max, mut inline) = if mode == Mode::IgnoreInline {
            (transfer_min, transfer_max, None)
        } else {
            self.inline_constraints(mode, block, (transfer_min, transfer_max))
        };
        if let (Some(inline), Some(block)) = (inline, block) {
            return (inline, block);
        }
        if self.natural.is_none() && inline.is_none() && block.is_none() {
            inline = Some(clamp(self.stretch_fit(), inline_min, inline_max));
        }
        if let Some(inline) = inline {
            let block = match self.ratio {
                Some(_) => self.block_from_ratio(inline),
                None => self.natural_or_default().1,
            };
            return (inline, clamp(block, block_min, block_max));
        }
        if let Some(block) = block {
            let inline = match self.ratio {
                Some(_) => self.inline_from_ratio(block),
                None => self.natural_or_default().0,
            };
            return (clamp(inline, inline_min, inline_max), block);
        }
        let natural = self.natural_or_default();
        (
            clamp(natural.0, inline_min, inline_max),
            clamp(natural.1, block_min, block_max),
        )
    }
    /** Block min, max and the resolved main block size, if any. */
    fn block_constraints(&self, mode: Mode) -> (f64, f64, Option<f64>) {
        if mode == Mode::IgnoreBlock {
            return (0.0, UNBOUNDED, None);
        }
        let [min_fit, max_fit] = self.node.style.height_bounds_fit;
        let min = match min_fit {
            true => self.intrinsic_block(mode),
            false => self.block_length(self.node.style.min_height, Some(0.0)),
        };
        let min = min.unwrap_or(self.edges.1);
        let max = match max_fit {
            true => self.intrinsic_block(mode),
            false => self.block_length(self.node.style.max_height, self.block_basis),
        };
        let max = max.unwrap_or(UNBOUNDED).max(min);
        let main = match self.node.style.height_fit {
            true => self.intrinsic_block(mode),
            false => self.block_length(self.node.style.height, self.block_basis),
        };
        let main = main.or_else(|| self.intrinsic_block(mode));
        (min, max, main.map(|size| clamp(size, min, max)))
    }
    /** The block size an `auto` or unresolvable height takes, and what a content keyword
    (`min-content`, `max-content`, `fit-content`) on a block property resolves to: a replaced
    box has one intrinsic block size (Blink's BlockSizeFunc). */
    fn intrinsic_block(&self, mode: Mode) -> Option<f64> {
        match self.ratio {
            None => self.natural.map(|natural| natural.1),
            Some(_) if mode == Mode::Normal => Some(self.size(Mode::IgnoreBlock).1),
            Some(_) => self.natural.map(|natural| natural.1),
        }
    }
    /** Inline min, max and the resolved main inline size, transferring the block limits when the
    inline size stays unresolved. */
    fn inline_constraints(
        &self,
        mode: Mode,
        block: Option<f64>,
        transferred: (f64, f64),
    ) -> (f64, f64, Option<f64>) {
        if let Some(fixed) = self.fixed {
            return (fixed, fixed, Some(fixed));
        }
        let mut min = self.min_inline(mode, block);
        let mut max = self.max_inline(mode, block).max(min);
        let main = self
            .main_inline(mode, block)
            .or_else(|| self.intrinsic_inline(mode, block));
        let inline = main.map(|size| clamp(size, min, max));
        if inline.is_none() {
            min = min.max(transferred.0.min(max));
            max = max.min(transferred.1);
        }
        (min, max, inline)
    }
    /** The inline size an `auto` or unresolvable width takes, and what a content keyword on an
    inline property resolves to: its min- and max-content widths are one size (Blink's
    MinMaxSizesFunc). */
    fn intrinsic_inline(&self, mode: Mode, block: Option<f64>) -> Option<f64> {
        if self.ratio.is_none() {
            return self.natural.map(|natural| natural.0);
        }
        if let Some(block) = block {
            return Some(self.inline_from_ratio(block));
        }
        self.natural?;
        if mode == Mode::Normal {
            Some(self.size(Mode::IgnoreInline).0)
        } else {
            self.natural.map(|natural| natural.0)
        }
    }
    /** Block min/max sizes carried to the inline axis through the ratio. */
    fn transferred(&self, block_min: f64, block_max: f64) -> (f64, f64) {
        if self.ratio.is_none() {
            return (0.0, UNBOUNDED);
        }
        let min = if block_min > 0.0 {
            self.inline_from_ratio(block_min)
        } else {
            0.0
        };
        let max = if block_max.is_finite() {
            self.inline_from_ratio(block_max)
        } else {
            UNBOUNDED
        };
        (min, max.max(min))
    }
    /** An authored block length as a border-box size; None for `auto` or a percentage of an
    indefinite height. */
    fn block_length(&self, extent: Option<Extent>, basis: Option<f64>) -> Option<f64> {
        let value = extent?.resolve_size(basis)?;
        Some(super::border_height(self.node, value))
    }
    /** The authored width as a border-box size; None for `auto` or a percentage of an indefinite
    width. */
    fn main_inline(&self, mode: Mode, block: Option<f64>) -> Option<f64> {
        let style = &self.node.style;
        if style.size_keywords[0] != SizeKeyword::None {
            return self.intrinsic_inline(mode, block);
        }
        if style.width_stretch {
            // `width: stretch` is the border box filling the available width less margins.
            return self.available.map(|_| self.stretch_fit());
        }
        let width = style.width?;
        let value = if style.percent_width {
            layout_unit_floor(self.available? * width / 100.0) + style.width_offset
        } else {
            width
        };
        Some(self.inline_border(value))
    }
    /** The min-width as a border-box size; `auto` is the edges alone. */
    fn min_inline(&self, mode: Mode, block: Option<f64>) -> f64 {
        if self.node.style.size_keywords[1] != SizeKeyword::None {
            return self.intrinsic_inline(mode, block).unwrap_or(self.edges.0);
        }
        self.node
            .style
            .min_width
            .map_or(self.edges.0, |width| self.inline_border(width))
    }
    /** The max-width as a border-box size; `none` or an unresolvable percentage is unbounded. */
    fn max_inline(&self, mode: Mode, block: Option<f64>) -> f64 {
        let style = &self.node.style;
        if style.size_keywords[2] != SizeKeyword::None {
            return self.intrinsic_inline(mode, block).unwrap_or(UNBOUNDED);
        }
        let Some(value) = style.max_width else {
            return UNBOUNDED;
        };
        if !style.percent_max_width {
            return self.inline_border(value);
        }
        self.available.map_or(UNBOUNDED, |available| {
            self.inline_border(available * value / 100.0 + style.max_width_offset)
        })
    }
    /** An authored width in the node's box-sizing as a border-box width. */
    fn inline_border(&self, value: f64) -> f64 {
        content_size(self.node, value.max(0.0), true) + self.edges.0
    }
    /** Fills the available width when only a ratio is known (Blink's StretchFit). */
    fn stretch_fit(&self) -> f64 {
        let margins = self.node.style.margin[1] + self.node.style.margin[3];
        match self.available {
            Some(available) => (available - margins).max(self.edges.0),
            None if self.node.style.percent_width => DEFAULT_NATURAL.0 + self.edges.0,
            None => self.edges.0,
        }
    }
    /** The natural size, or the edges alone for a box with neither natural size nor ratio. */
    fn natural_or_default(&self) -> (f64, f64) {
        self.natural.unwrap_or(self.edges)
    }
    /** Blink's InlineSizeFromAspectRatio. */
    fn inline_from_ratio(&self, block: f64) -> f64 {
        let (ratio, border_box) = self.ratio.unwrap_or((1.0, false));
        if border_box {
            (block * ratio).max(self.edges.0)
        } else {
            (block - self.edges.1).max(0.0) * ratio + self.edges.0
        }
    }
    /** Blink's BlockSizeFromAspectRatio. */
    fn block_from_ratio(&self, inline: f64) -> f64 {
        let (ratio, border_box) = self.ratio.unwrap_or((1.0, false));
        if border_box {
            (inline / ratio).max(self.edges.1)
        } else {
            (inline - self.edges.0).max(0.0) / ratio + self.edges.1
        }
    }
}

/**
 * The used aspect ratio, Blink's GetReplacedAspectRatio: an authored `<ratio>` wins and follows
 * box-sizing; `auto && <ratio>` and `auto` use the natural ratio of the content box when there is
 * one. An image not yet decoded takes its ratio from its HTML width and height, which Blink maps
 * to `aspect-ratio: auto w / h`.
 */
fn ratio(node: &Node, natural: Option<f64>) -> Option<(f64, bool)> {
    // Size containment drops the content's ratio but not the attributes' `auto w / h` hint,
    // which an svg's own width and height attributes do not give.
    let hinted = || {
        let hinted = node.tag != "svg";
        Some(dimension(node, "width").filter(|_| hinted)? / dimension(node, "height")?)
    };
    let natural = natural.or_else(hinted);
    let border_box = node.style.box_sizing == BoxSizing::BorderBox;
    match (node.style.aspect_ratio, natural) {
        (Some(_), Some(natural)) if node.style.aspect_ratio_auto => Some((natural, false)),
        (Some(authored), _) => Some((authored, border_box)),
        (None, natural) => natural.map(|natural| (natural, false)),
    }
}
/** The natural size of a box with no natural size and no ratio: an img still loading has none,
other replaced content has CSS 2.1's 300 by 150 default. */
fn unloaded(node: &Node, edges: (f64, f64)) -> (f64, f64) {
    if node.tag == "img" {
        edges
    } else {
        (DEFAULT_NATURAL.0 + edges.0, DEFAULT_NATURAL.1 + edges.1)
    }
}
/** HTML dimensions accept only positive integers. */
fn dimension(node: &Node, name: &str) -> Option<f64> {
    node.attributes
        .get(name)?
        .parse::<u32>()
        .ok()
        .filter(|value| *value > 0)
        .map(f64::from)
}
