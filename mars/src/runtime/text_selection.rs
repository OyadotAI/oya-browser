/** Page text selection over Eye's painted lines: hit testing, word and line expansion, copied text and highlight boxes. Pure geometry; the shell supplies text measurement. */
use crate::document::{DisplayList, Document};

/// Controls keep their own native editing and selection; secret values are never selectable.
const CONTROL_TAGS: [&str; 5] = ["input", "textarea", "select", "option", "button"];
/// CSS `line-height: normal` when Eye did not record the used advance.
const NORMAL_LINE: f64 = crate::constants::CSS_NORMAL_LINE_HEIGHT;

/** One painted line of page text in document points, with the display item it came from. */
#[derive(Clone, Debug, PartialEq)]
pub struct TextLine {
    /// Literal rendered text.
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// Index of the display item, so the shell can measure with its font.
    pub item: usize,
}

/** A position between two characters of one line. */
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Caret {
    /// Line index in reading (document) order.
    pub line: usize,
    /// Character offset within the line, 0..=chars.
    pub offset: usize,
}

/** Every selectable painted line, in document order; form controls, secret fields and transformed text are left out. */
pub fn lines(display: &DisplayList, document: &Document) -> Vec<TextLine> {
    let excluded = |local_id: usize| {
        document
            .nodes
            .iter()
            .find(|node| node.local_id == local_id)
            .is_some_and(|node| {
                CONTROL_TAGS.contains(&node.tag.as_str())
                    || node.secret()
                    || node.input_type() == "password"
            })
    };
    let mut result = Vec::new();
    for (index, item) in display.items.iter().enumerate() {
        if item.lines.is_empty()
            || item.transform.is_some()
            || CONTROL_TAGS.contains(&item.tag.as_str())
            || excluded(item.local_id)
        {
            continue;
        }
        let height = item.line_height.unwrap_or(item.font_size * NORMAL_LINE);
        for line in &item.lines {
            result.push(TextLine {
                text: line.text.clone(),
                x: line.x,
                y: line.y,
                width: line.width,
                height,
                item: index,
            });
        }
    }
    result
}

/** The caret nearest a point: the line on that row closest horizontally, else the nearest row; then the character boundary nearest the point. */
pub fn hit(
    lines: &[TextLine],
    x: f64,
    y: f64,
    measure: &dyn Fn(usize, &str) -> f64,
) -> Option<Caret> {
    let distance = |line: &TextLine| {
        let dy = if y < line.y {
            line.y - y
        } else if y > line.y + line.height {
            y - line.y - line.height
        } else {
            0.0
        };
        let dx = if x < line.x {
            line.x - x
        } else if x > line.x + line.width {
            x - line.x - line.width
        } else {
            0.0
        };
        (dy, dx)
    };
    let (index, line) = lines.iter().enumerate().min_by(|a, b| {
        let (ay, ax) = distance(a.1);
        let (by, bx) = distance(b.1);
        ay.total_cmp(&by).then(ax.total_cmp(&bx))
    })?;
    Some(Caret {
        line: index,
        offset: offset_at(line, x - line.x, measure),
    })
}

/** The character boundary nearest `dx` points into the line, by binary search over measured prefixes. */
fn offset_at(line: &TextLine, dx: f64, measure: &dyn Fn(usize, &str) -> f64) -> usize {
    let bounds: Vec<usize> = line
        .text
        .char_indices()
        .map(|(at, _)| at)
        .chain(std::iter::once(line.text.len()))
        .collect();
    let width = |chars: usize| measure(line.item, &line.text[..bounds[chars]]);
    let (mut low, mut high) = (0, bounds.len() - 1);
    while low < high {
        let middle = (low + high).div_ceil(2);
        if width(middle) <= dx {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    if low + 1 < bounds.len() && dx - width(low) > (width(low + 1) - width(low)) / 2.0 {
        low + 1
    } else {
        low
    }
}

/** Double-click: the run of letters and digits around the caret, or the one character there. */
pub fn word(lines: &[TextLine], caret: Caret) -> (Caret, Caret) {
    let chars: Vec<char> = lines[caret.line].text.chars().collect();
    let at = caret.offset.min(chars.len().saturating_sub(1));
    let wordy = |c: char| c.is_alphanumeric() || c == '_' || c == '\'';
    if chars.is_empty() || !wordy(chars[at]) {
        let end = (at + 1).min(chars.len());
        return (
            Caret {
                line: caret.line,
                offset: at,
            },
            Caret {
                line: caret.line,
                offset: end,
            },
        );
    }
    let start = (0..=at)
        .rev()
        .take_while(|&i| wordy(chars[i]))
        .last()
        .unwrap_or(at);
    let end = (at..chars.len())
        .take_while(|&i| wordy(chars[i]))
        .last()
        .map_or(at, |i| i + 1);
    (
        Caret {
            line: caret.line,
            offset: start,
        },
        Caret {
            line: caret.line,
            offset: end,
        },
    )
}

/** Triple-click: the whole painted line. */
pub fn whole_line(lines: &[TextLine], caret: Caret) -> (Caret, Caret) {
    let end = lines[caret.line].text.chars().count();
    (
        Caret {
            line: caret.line,
            offset: 0,
        },
        Caret {
            line: caret.line,
            offset: end,
        },
    )
}

/** The selected text in reading order; lines of one block join with a space, blocks with a newline. */
pub fn text(lines: &[TextLine], a: Caret, b: Caret) -> String {
    let (start, end) = if a <= b { (a, b) } else { (b, a) };
    let mut out = String::new();
    for index in start.line..=end.line.min(lines.len().saturating_sub(1)) {
        let line = &lines[index];
        let from = if index == start.line { start.offset } else { 0 };
        let to = if index == end.line {
            end.offset
        } else {
            usize::MAX
        };
        let piece: String = line
            .text
            .chars()
            .skip(from)
            .take(to.saturating_sub(from))
            .collect();
        if index > start.line {
            out.push(if lines[index - 1].item == line.item {
                ' '
            } else {
                '\n'
            });
        }
        out.push_str(&piece);
    }
    out
}

/** Highlight boxes (x, y, width, height) for a selection, one per touched line. */
pub fn rects(
    lines: &[TextLine],
    a: Caret,
    b: Caret,
    measure: &dyn Fn(usize, &str) -> f64,
) -> Vec<[f64; 4]> {
    let (start, end) = if a <= b { (a, b) } else { (b, a) };
    let prefix = |line: &TextLine, chars: usize| {
        let at = line
            .text
            .char_indices()
            .nth(chars)
            .map_or(line.text.len(), |(at, _)| at);
        measure(line.item, &line.text[..at])
    };
    (start.line..=end.line.min(lines.len().saturating_sub(1)))
        .filter_map(|index| {
            let line = &lines[index];
            let left = if index == start.line {
                prefix(line, start.offset)
            } else {
                0.0
            };
            let right = if index == end.line {
                prefix(line, end.offset)
            } else {
                line.width
            };
            (right > left).then_some([line.x + left, line.y, right - left, line.height])
        })
        .collect()
}
