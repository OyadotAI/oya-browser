/** Replaced-element sizing, a port of Blink's ComputeReplacedSize (layout/length_utils.cc).
 *
 * Follows CSS 2.1 10.3.2, 10.4, 10.6.2 and 10.7 with css-sizing-4's aspect-ratio transfers:
 * natural sizes and the aspect ratio are normalised to the border box, each axis resolves its
 * main size and min/max constraints, an automatic axis takes its size from the other one through
 * the ratio, and min/max constraints on one axis transfer through the ratio to the other. All
 * sizes here are border-box sizes, as in Blink. */
use super::{content_size, horizontal_edges, layout_unit_floor, vertical_edges};
use crate::document::{BoxSizing, Node, style::Extent};

/** No upper bound, Blink's `LayoutUnit::Max()`. */
const UNBOUNDED: f64 = f64::INFINITY;
/** The default natural size of a replaced box with neither natural size nor ratio (CSS 2.1 10.3.2). */
const DEFAULT_NATURAL: (f64, f64) = (300.0, 150.0);

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

/**
 * Min- and max-content contributions of a replaced box, content box only, after Blink's
 * ComputeMinAndMaxContentContributionForReplaced: the max is its size at an indefinite width, and
 * a percentage width or max-width makes it compressible so its min shrinks to its min-width (an
 * `max-width: 100%` image no longer props its grid track or table column open).
 */
pub(in crate::document::render) fn replaced_contribution(node: &Node) -> (f64, f64) {
    let edges = horizontal_edges(node);
    let max = (replaced_size(node, None, None).0 - edges).max(0.0);
    let compressible = node.style.percent_width || node.style.percent_max_width;
    let min = if compressible {
        let space = Replaced::new(node, None, None);
        (space.min_inline() - edges).max(0.0).min(max)
    } else {
        max
    };
    (min, max)
}

impl<'a> Replaced<'a> {
    /** Normalises the ratio and natural size as Blink's ComputeNormalizedNaturalSize does. */
    fn new(node: &'a Node, available: Option<f64>, block_basis: Option<f64>) -> Self {
        let edges = (horizontal_edges(node), vertical_edges(node));
        let natural = node
            .image_size
            .filter(|(w, h)| *w > 0 && *h > 0)
            .map(|(w, h)| (f64::from(w), f64::from(h)));
        let mut space = Self {
            node,
            available,
            block_basis,
            edges,
            ratio: ratio(node, natural),
            natural: None,
        };
        space.natural = match (natural, space.ratio) {
            (Some((w, _)), Some(_)) => {
                let inline = w + edges.0;
                Some((inline, space.block_from_ratio(inline)))
            }
            (Some((w, h)), None) => Some((w + edges.0, h + edges.1)),
            (None, Some(_)) => None,
            (None, None) => Some(unloaded(node, edges)),
        };
        space
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
        let min = self.block_length(self.node.style.min_height, Some(0.0));
        let min = min.unwrap_or(self.edges.1);
        let max = self
            .block_length(self.node.style.max_height, self.block_basis)
            .unwrap_or(UNBOUNDED)
            .max(min);
        let main = self
            .block_length(self.node.style.height, self.block_basis)
            .or_else(|| self.intrinsic_block(mode));
        (min, max, main.map(|size| clamp(size, min, max)))
    }
    /** The block size an `auto` or unresolvable height takes, Blink's BlockSizeFunc. */
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
        let mut min = self.min_inline();
        let mut max = self.max_inline().max(min);
        let main = self
            .main_inline()
            .or_else(|| self.intrinsic_inline(mode, block));
        let inline = main.map(|size| clamp(size, min, max));
        if inline.is_none() {
            min = min.max(transferred.0.min(max));
            max = max.min(transferred.1);
        }
        (min, max, inline)
    }
    /** The inline size an `auto` or unresolvable width takes, Blink's MinMaxSizesFunc. */
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
    fn main_inline(&self) -> Option<f64> {
        let style = &self.node.style;
        let width = style.width?;
        let value = if style.percent_width {
            layout_unit_floor(self.available? * width / 100.0) + style.width_offset
        } else {
            width
        };
        Some(self.inline_border(value))
    }
    /** The min-width as a border-box size; `auto` is the edges alone. */
    fn min_inline(&self) -> f64 {
        self.node
            .style
            .min_width
            .map_or(self.edges.0, |width| self.inline_border(width))
    }
    /** The max-width as a border-box size; `none` or an unresolvable percentage is unbounded. */
    fn max_inline(&self) -> f64 {
        let style = &self.node.style;
        let Some(value) = style.max_width else {
            return UNBOUNDED;
        };
        if !style.percent_max_width {
            return self.inline_border(value);
        }
        self.available.map_or(UNBOUNDED, |available| {
            self.inline_border(available * value / 100.0)
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
fn ratio(node: &Node, natural: Option<(f64, f64)>) -> Option<(f64, bool)> {
    let natural = natural
        .map(|(w, h)| w / h)
        .or_else(|| Some(dimension(node, "width")? / dimension(node, "height")?));
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
