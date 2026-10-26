/** Text chains, which both painters use: text that layout split into runs (inline boxes, separate text nodes) sits on
 * one line in one face, and Blink shapes and draws such a line as one result sliced per item, so
 * its glyphs land where one string's would. CoreText quantizes a run's glyph positions from the
 * run's own origin, so separately drawn runs land a fraction of a pixel off the joined string
 * (inline-formatting-context-001's three "Filler Text" spans against one text). A chain of runs
 * that abut, or are separated only by whole spaces, is rasterized once as the joined text, and
 * each run paints that coverage in its own color over the columns from its start to the next
 * run's. The windowless raster and the desktop canvas both draw chains this way. */
use super::{DisplayItem, PaintLine};
use crate::platform::Face;
use std::collections::HashMap;

/** How far a gap may be from a whole number of spaces and still be white space between runs. */
const GAP_SLACK: f64 = 0.02;
/** How far, per joint between runs, the joined text's own width may be from the runs' laid-out
 * extent: kerning across a run boundary moves it by a pixel or two, justification or tabs by
 * more. */
const SHAPING_SLACK: f64 = 2.0;
/** The most spaces a gap between chained runs holds; wider gaps are layout, not text. */
const MAX_SPACES: f64 = 4.0;

/** One joined text: what its runs draw together, from the first run's origin. */
pub struct TextChain {
    /** The runs' text joined with the spaces between them. */
    pub text: String,
    /** The first run's left edge. */
    pub x: f64,
    /** From the first run's start to the last run's end. */
    pub width: f64,
}
/** One run's share of a chain: the chain and the document x range whose pixels it paints. */
#[derive(Clone, Copy)]
pub struct ChainMember {
    /** Index of its chain in `TextChains::chains`. */
    pub chain: usize,
    /** The document x where its pixels start (minus infinity for the first run). */
    pub from: f64,
    /** The document x where the next run's pixels start (infinity for the last run). */
    pub to: f64,
}
/** Every chain among a display list's runs and each chained run's share. */
#[derive(Default)]
pub struct TextChains {
    /** The joined texts. */
    pub chains: Vec<TextChain>,
    /** Each chained run's share, by `chain_key`. */
    pub members: HashMap<ChainKey, ChainMember>,
}
/** A run's identity while it paints: its item and its line's position (painting draws copies
 * of the display list's items, so addresses do not identify them). */
pub type ChainKey = (usize, u64, u64);

/** One run with what decides whether it joins its neighbors. */
struct Run<'a> {
    item: &'a DisplayItem,
    line: &'a PaintLine,
    /** Its line's top and line height: runs of one line box in one face share a baseline. */
    baseline: (f64, f64),
}

/** The chains among `items`' runs. */
pub fn text_chains<'a>(items: impl Iterator<Item = &'a DisplayItem>) -> TextChains {
    let mut runs: Vec<Run<'a>> = items.filter(|item| joinable(item)).flat_map(runs).collect();
    runs.sort_by(|a, b| {
        (a.baseline.0.total_cmp(&b.baseline.0))
            .then(a.baseline.1.total_cmp(&b.baseline.1))
            .then(a.line.x.total_cmp(&b.line.x))
    });
    let mut out = TextChains::default();
    let mut start = 0;
    for end in 1..=runs.len() {
        if end < runs.len() && joins(&runs[end - 1], &runs[end]) {
            continue;
        }
        record(&runs[start..end], &mut out);
        start = end;
    }
    out
}
/** An item's runs that can join: those with ink and no right-to-left text. */
fn runs(item: &DisplayItem) -> impl Iterator<Item = Run<'_>> {
    let height = item.line_height.unwrap_or(item.font_size);
    item.lines
        .iter()
        .filter(|line| !line.text.trim().is_empty() && !line.text.chars().any(bidi))
        .map(move |line| Run {
            item,
            line,
            baseline: (line.y, height),
        })
}
/** A right-to-left letter or a bidi control: such runs are drawn in an order bidi.rs chose, which
 * a joined string would reorder. */
fn bidi(c: char) -> bool {
    matches!(
        u32::from(c),
        0x0590..=0x08FF
            | 0xFB1D..=0xFDFF
            | 0xFE70..=0xFEFF
            | 0x10800..=0x10FFF
            | 0x1E800..=0x1EFFF
            | 0x200E..=0x200F
            | 0x202A..=0x202E
            | 0x2066..=0x2069
    )
}
/** Whether an item's runs may share a drawing: horizontal and untransformed. A right-to-left
 * paragraph's runs may join too, since only runs without right-to-left text join, and those
 * keep their visual order. */
fn joinable(item: &DisplayItem) -> bool {
    item.writing == 0
        && item.transform.is_none()
        && item.word_spacing == 0.0
        && !item.lines.is_empty()
}
/** Whether `next` continues `prev` on its line: the same inline formatting context, baseline,
 * face and clip, and only whole spaces between them. */
fn joins(prev: &Run<'_>, next: &Run<'_>) -> bool {
    let (a, b) = (prev.item, next.item);
    a.inline_root == b.inline_root
        && prev.baseline == next.baseline
        && face(a) == face(b)
        && a.clip == b.clip
        && spaces(prev, next).is_some()
}
/** How many spaces fill the gap between two runs, if a whole number of them does. */
fn spaces(prev: &Run<'_>, next: &Run<'_>) -> Option<usize> {
    let gap = next.line.x - (prev.line.x + prev.line.width);
    if gap.abs() <= GAP_SLACK {
        return Some(0);
    }
    let space = crate::platform::measure(" ", face(prev.item));
    if space <= 0.0 {
        return None;
    }
    let count = (gap / space).round();
    let whole = (1.0..=MAX_SPACES).contains(&count) && (gap - count * space).abs() <= GAP_SLACK;
    whole.then_some(count as usize)
}
/** Records one chain of two or more runs whose joined text draws where they were laid out. */
fn record(chain: &[Run<'_>], out: &mut TextChains) {
    let [head, rest @ ..] = chain else {
        return;
    };
    let Some(last) = rest.last() else {
        return;
    };
    let mut text = head.line.text.clone();
    for pair in chain.windows(2) {
        text.push_str(&" ".repeat(spaces(&pair[0], &pair[1]).unwrap_or(0)));
        text.push_str(&pair[1].line.text);
    }
    let width = last.line.x + last.line.width - head.line.x;
    // Justified or otherwise spread runs are not where the joined text would draw its glyphs.
    let natural = crate::platform::measure(&text, face(head.item));
    if (natural - width).abs() > SHAPING_SLACK * rest.len() as f64 {
        return;
    }
    let index = out.chains.len();
    for (at, run) in chain.iter().enumerate() {
        let from = if at == 0 {
            f64::NEG_INFINITY
        } else {
            run.line.x
        };
        let to = chain.get(at + 1).map_or(f64::INFINITY, |next| next.line.x);
        let member = ChainMember {
            chain: index,
            from,
            to,
        };
        out.members.insert(chain_key(run.item, run.line), member);
    }
    out.chains.push(TextChain {
        text,
        x: head.line.x,
        width,
    });
}
/** The face an item measured its runs with. */
fn face(item: &DisplayItem) -> Face {
    Face {
        size: item.font_size,
        weight: item.font_weight,
        family: item.font_family,
        spacing: item.letter_spacing,
        italic: item.italic,
        autospace: item.autospace,
        upright: item.upright,
    }
}
/** A run's key: its item's identity and its line's position. */
pub fn chain_key(item: &DisplayItem, line: &PaintLine) -> ChainKey {
    (item.local_id, line.x.to_bits(), line.y.to_bits())
}
