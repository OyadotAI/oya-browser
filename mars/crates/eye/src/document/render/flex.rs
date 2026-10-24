/** Source-guided bounded flexible-length resolution; Chromium 2024 BSD-style routines inform the freeze rules. */
use crate::{
    constants::{LAYOUT_EPSILON, LAYOUT_EXTENT},
    protocol::{EngineError, EngineResult},
};

/** One item's content sizes remain distinct from margins and border/padding. */
#[derive(Clone, Debug)]
pub(super) struct FlexItem {
    pub local_id: usize,
    pub base_content: f64,
    pub hypothetical_content: f64,
    pub min_content: f64,
    pub max_content: f64,
    pub margin_start: f64,
    pub margin_end: f64,
    /// Main-axis `auto` margins (start, end); they absorb the line's positive free space
    /// before `justify-content` sees any (Flexbox 9.5), which is how a nav pushes itself right.
    pub auto_margins: [bool; 2],
    pub main_edges: f64,
    pub grow: f64,
    pub shrink: f64,
}
impl FlexItem {
    /** Noncontent space never receives a grow or shrink factor. */
    pub fn outer(&self, content: f64) -> f64 {
        content + self.margin_start + self.margin_end + self.main_edges
    }
}

/** Positive free space split equally over the line's auto margins; zero when there are none
or the line overflows, where auto margins collapse to zero and alignment proceeds as usual. */
pub(super) fn auto_margin_share(items: &[FlexItem], free: f64) -> f64 {
    let count = items
        .iter()
        .map(|item| item.auto_margins.iter().filter(|auto| **auto).count())
        .sum::<usize>();
    if count == 0 || free <= 0.0 {
        0.0
    } else {
        free / count as f64
    }
}

/** Cross-axis offset of an item whose cross auto margins take the free space, overriding its
`align-self`; None when neither cross margin is auto. */
pub(super) fn auto_cross_offset(auto_start: bool, auto_end: bool, free: f64) -> Option<f64> {
    match (auto_start, auto_end) {
        (false, false) => None,
        (true, true) => Some(free / 2.0),
        (true, false) => Some(free),
        (false, true) => Some(0.0),
    }
}

/** Greedy lines use hypothetical outer sizes and a gap only between neighboring items. */
pub(super) fn break_lines(
    items: &[FlexItem],
    inner: f64,
    gap: f64,
    wrap: bool,
) -> Vec<std::ops::Range<usize>> {
    let mut lines = Vec::new();
    let (mut start, mut used) = (0, 0.0);
    for (index, item) in items.iter().enumerate() {
        let next = item.outer(item.hypothetical_content) + if index > start { gap } else { 0.0 };
        if wrap && index > start && used + next > inner + LAYOUT_EPSILON {
            lines.push(start..index);
            start = index;
            used = item.outer(item.hypothetical_content);
        } else {
            used += next;
        }
    }
    if start < items.len() {
        lines.push(start..items.len());
    }
    lines
}

/** Each iteration freezes a violation or finishes; no input can create an unbounded retry. */
pub(super) fn resolve_line(
    items: &[FlexItem],
    inner_main: f64,
    gap: f64,
) -> EngineResult<Vec<f64>> {
    let occupied = items
        .iter()
        .map(|item| item.outer(item.hypothetical_content))
        .sum::<f64>()
        + gap * items.len().saturating_sub(1) as f64;
    let grow = occupied < inner_main;
    let mut sizes = items
        .iter()
        .map(|item| item.hypothetical_content)
        .collect::<Vec<_>>();
    let mut frozen = items
        .iter()
        .map(|item| {
            let factor = if grow { item.grow } else { item.shrink };
            factor == 0.0
                || if grow {
                    item.base_content > item.hypothetical_content
                } else {
                    item.base_content < item.hypothetical_content
                }
        })
        .collect::<Vec<_>>();
    let initial = free_space(items, &sizes, &frozen, inner_main, gap);
    for _ in 0..=items.len() {
        if frozen.iter().all(|frozen| *frozen) {
            return Ok(sizes);
        }
        let violation = distribute(items, &mut sizes, &frozen, inner_main, gap, initial, grow)?;
        let total = violation.iter().sum::<f64>();
        if total.abs() <= LAYOUT_EPSILON {
            return Ok(sizes);
        }
        let mut progress = false;
        for (index, violation) in violation.iter().enumerate() {
            if !frozen[index]
                && (total > 0.0 && *violation > 0.0 || total < 0.0 && *violation < 0.0)
            {
                frozen[index] = true;
                progress = true;
            }
        }
        if !progress {
            return Err(invalid());
        }
    }
    Err(invalid())
}

/** Frozen items contribute used sizes while unfrozen items restart from their flex bases. */
fn free_space(items: &[FlexItem], sizes: &[f64], frozen: &[bool], inner: f64, gap: f64) -> f64 {
    inner
        - gap * items.len().saturating_sub(1) as f64
        - items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                item.outer(if frozen[index] {
                    sizes[index]
                } else {
                    item.base_content
                })
            })
            .sum::<f64>()
}

/** Scaled shrink factors use base content; partial grow/shrink sums preserve unused free space. */
fn distribute(
    items: &[FlexItem],
    sizes: &mut [f64],
    frozen: &[bool],
    inner: f64,
    gap: f64,
    initial: f64,
    grow: bool,
) -> EngineResult<Vec<f64>> {
    let factor = |item: &FlexItem| if grow { item.grow } else { item.shrink };
    let total = items
        .iter()
        .enumerate()
        .filter(|(i, _)| !frozen[*i])
        .map(|(_, item)| factor(item))
        .sum::<f64>();
    let weighted = items
        .iter()
        .enumerate()
        .filter(|(i, _)| !frozen[*i])
        .map(|(_, item)| {
            if grow {
                item.grow
            } else {
                item.shrink * item.base_content
            }
        })
        .sum::<f64>();
    let mut free = free_space(items, sizes, frozen, inner, gap);
    if total > 0.0 && total < 1.0 && (initial * total).abs() < free.abs() {
        free = initial * total;
    }
    let mut violations = vec![0.0; items.len()];
    for (index, item) in items.iter().enumerate() {
        if frozen[index] {
            continue;
        }
        let share = if weighted > 0.0 {
            if grow {
                item.grow / weighted
            } else {
                item.shrink * item.base_content / weighted
            }
        } else {
            0.0
        };
        let proposed = item.base_content + free * share;
        let used = proposed
            .max(item.min_content)
            .min(item.max_content)
            .max(0.0);
        if !used.is_finite() || used > LAYOUT_EXTENT {
            return Err(invalid());
        }
        sizes[index] = used;
        violations[index] = used - proposed;
    }
    Ok(violations)
}

/** Nonfinite arithmetic never reaches native geometry or a committed page. */
fn invalid() -> EngineError {
    EngineError::unsupported("The flex layout exceeds the supported numeric bounds.")
}
