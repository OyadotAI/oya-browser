/** Source-guided bounded flexible-length resolution; Chromium 2024 BSD-style routines inform the freeze rules. */
use crate::{
    constants::{LAYOUT_EPSILON, LAYOUT_EXTENT},
    protocol::{EngineError, EngineResult},
};

/** One item's content sizes remain distinct from margins and border/padding. */
#[derive(Clone, Debug)]
pub(super) struct FlexItem {
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

/** A scroll container's automatic minimum size is zero (css-grid-2 6.6, Blink's
 * `ShouldApplyMinSizeAutoForChild`), so `overflow: hidden` items never widen their tracks. */
pub(super) fn scroll_container(node: &crate::document::Node) -> bool {
    node.style.overflow != crate::document::style::Overflow::Visible
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

/** How a container breaks its items into lines: one line, greedy wrapping, or balanced
wrapping with a minimum line count (`flex-wrap: balance` with `flex-line-count`). */
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Breaking {
    Single,
    Greedy,
    Balance(u32),
}
impl Breaking {
    /** The container's mode as Blink reads it from `ResolvedIsFlexNowrap` and
    `ResolvedFlexLineCount`. */
    pub fn of(node: &crate::document::Node) -> Self {
        let style = &node.style;
        if style.flex_wrap == crate::document::FlexWrap::NoWrap || style.webkit_box {
            Self::Single
        } else if style.flex_balance {
            Self::Balance(style.flex_line_count.unwrap_or(1))
        } else {
            Self::Greedy
        }
    }
}

/** Lines in the container's breaking mode (Blink's `BreakFlexItemsIntoLines`). */
pub(super) fn break_lines_in(
    items: &[FlexItem],
    inner: f64,
    gap: f64,
    breaking: Breaking,
) -> Vec<std::ops::Range<usize>> {
    match breaking {
        Breaking::Balance(min) if !items.is_empty() => balance(items, inner, gap, min),
        _ => break_lines(items, inner, gap, breaking != Breaking::Single),
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

/** Balanced breaking works in Blink's `LayoutUnit` steps (1/64 px), so ties between candidate
breaks fall the same way they do in Chromium. */
const BALANCE_UNITS_PER_PX: f64 = 64.0;

/** A length in balance units; an infinite or huge break size clamps to the layout bound. */
fn balance_units(px: f64) -> u64 {
    (px.clamp(0.0, LAYOUT_EXTENT) * BALANCE_UNITS_PER_PX).round() as u64
}

/** Prefix sums of margin-box sizes plus one gap each, the line-break size and the gap: the
state Blink's `BalanceBreakFlexItemsIntoLines` scores breaks over. */
struct Balance {
    sums: Vec<u64>,
    size: u64,
    gap: u64,
}
impl Balance {
    /** Length of items `start..=end` on one line, without the trailing gap. */
    fn length(&self, start: usize, end: usize) -> u64 {
        let before = if start == 0 { 0 } else { self.sums[start - 1] };
        self.sums[end]
            .saturating_sub(before)
            .saturating_sub(self.gap)
    }

    /** How many lines greedy packing makes at break size `size` (Blink's `GreedyLineCount`): a
    line ends before the first item that would overflow it, unless that item is alone. */
    fn greedy_count(&self, size: u64) -> usize {
        let (mut at, mut previous, mut lines) = (0usize, 0u64, 1usize);
        loop {
            let limit = previous.saturating_add(size).saturating_add(self.gap);
            let next = at + self.sums[at..].partition_point(|sum| *sum <= limit);
            if next == self.sums.len() {
                return lines;
            }
            let single = usize::from(lines == 1) + (next - at) <= 1;
            at = if single { next } else { next - 1 };
            previous = self.sums[at];
            if at + 1 != self.sums.len() {
                lines += 1;
            }
        }
    }

    /** Shrinks the break size by bisection until greedy packing reaches `min` lines, then grows
    items that end a line exactly by one unit, last first, until it does (Blink's
    `ApplyMinLineCount`; four equal items can never make exactly three lines otherwise). */
    fn apply_min_lines(&mut self, min: usize) -> usize {
        let total = self
            .sums
            .last()
            .copied()
            .unwrap_or(0)
            .saturating_sub(self.gap);
        let (mut low, mut high) = (0u64, self.size.min(total));
        while low < high {
            let mid = low + (high - low) / 2;
            if self.greedy_count(mid) > min {
                low = mid + 1;
            } else {
                high = mid;
            }
        }
        self.size = high;
        let mut count = self.greedy_count(self.size);
        if count >= min {
            return count;
        }
        for index in self.perfect_fits().into_iter().rev() {
            for sum in &mut self.sums[index..] {
                *sum = sum.saturating_add(1);
            }
            count = self.greedy_count(self.size);
            if count >= min {
                break;
            }
        }
        count
    }

    /** Items whose line, as greedy packing measures it, ends exactly at the break size. */
    fn perfect_fits(&self) -> Vec<usize> {
        let (mut previous, mut has_content, mut fits) = (0u64, false, Vec::new());
        for index in 0..self.sums.len() {
            let line = self.sums[index]
                .saturating_sub(previous)
                .saturating_sub(self.gap);
            if line == self.size {
                fits.push(index);
            }
            if line > self.size {
                previous = self.sums[if has_content { index - 1 } else { index }];
                has_content = false;
                continue;
            }
            has_content = true;
        }
        fits
    }

    /** For each end, the earliest start whose line still fits; an item too large for any line
    sits alone. */
    fn earliest_starts(&self) -> Vec<usize> {
        let mut starts = Vec::with_capacity(self.sums.len());
        let mut start = 0;
        for end in 0..self.sums.len() {
            while start < end && self.length(start, end) > self.size {
                start += 1;
            }
            starts.push(start);
            if start == end && self.length(start, end) > self.size {
                start += 1;
            }
        }
        starts
    }

    /** Minimum total of squared free space over every line ending at each item (Knuth-Plass for
    flex items, Blink's `Score`). An equal total keeps the later start, so earlier lines take the
    most items. An overflowing single item scores zero. Returns each end's best previous break. */
    fn best_breaks(&self) -> Vec<Option<usize>> {
        let starts = self.earliest_starts();
        let mut best = vec![u64::MAX; self.sums.len()];
        let mut breaks = vec![None; self.sums.len()];
        for end in 0..self.sums.len() {
            for start in starts[end]..=end {
                let length = self.length(start, end);
                let free = self.size.saturating_sub(length);
                let line = free.saturating_mul(free);
                if line > best[end] {
                    break;
                }
                let score = line.saturating_add(if start == 0 { 0 } else { best[start - 1] });
                if score <= best[end] {
                    best[end] = score;
                    breaks[end] = start.checked_sub(1);
                }
            }
        }
        breaks
    }
}

/** Balanced lines (css-flexbox-2 "balance"): at least `min` lines, never more than items, chosen
to minimise the summed squares of each line's free space. Negative margin boxes count as zero so
the prefix sums stay monotonic, as in Blink. ponytail: O(n^2) breaks per container, with Blink's
pruning; switch to Hirschberg-Larmore if containers with thousands of items show up. */
fn balance(items: &[FlexItem], inner: f64, gap: f64, min: u32) -> Vec<std::ops::Range<usize>> {
    let gap = balance_units(gap);
    let mut sums = Vec::with_capacity(items.len());
    let mut total = 0u64;
    for item in items {
        total = total
            .saturating_add(balance_units(item.outer(item.hypothetical_content)))
            .saturating_add(gap);
        sums.push(total);
    }
    let size = balance_units(inner);
    let mut state = Balance { sums, size, gap };
    let min = (min as usize).clamp(1, items.len());
    if state.greedy_count(state.size) < min {
        state.apply_min_lines(min);
    }
    let breaks = state.best_breaks();
    let mut ends = vec![items.len()];
    let mut at = breaks[items.len() - 1];
    while let Some(index) = at {
        ends.push(index + 1);
        at = breaks[index];
    }
    ends.reverse();
    let mut start = 0;
    ends.into_iter()
        .map(|end| {
            let range = start..end;
            start = end;
            range
        })
        .collect()
}
