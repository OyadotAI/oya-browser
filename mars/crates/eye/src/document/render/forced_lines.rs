/**
 * Preserved newlines inside inline children, for a container's max-content width: a child whose
 * `white-space` keeps newlines (`pre`, `pre-wrap`, `break-spaces`, `pre-line`) ends the line at
 * each one, as a `<br>` does, so only its first line joins what came before and only its last
 * line joins what follows (Blink's ComputeMinMaxSizes breaks at every forced break). Without it
 * `<span style="white-space:pre-wrap">AB&#10;</span><span> CD</span>` sized a float to "AB CD".
 */
use super::flow::*;
use crate::document::WhiteSpace;

/** How a child with preserved newlines splits its lines: the first line's width, the widest
 * line between, and the last line's width (None when the text ends at a newline). */
pub(super) struct Split {
    pub head: f64,
    pub middle: f64,
    pub tail: Option<f64>,
}

/** The split of `child` when it is a run of text that keeps newlines and holds one; None for
 * anything else, which joins its line whole. */
pub(super) fn split(context: &mut LayoutContext<'_>, child: &Node) -> EngineResult<Option<Split>> {
    let keeps = matches!(
        child.style.white_space,
        WhiteSpace::Pre | WhiteSpace::PreWrap | WhiteSpace::PreLine
    );
    let own = super::text::own_text(child);
    if !keeps || !own.contains('\n') || !context.children(child.local_id).is_empty() {
        return Ok(None);
    }
    let text = super::text::transformed(&child.style, &own).into_owned();
    let face = super::sizing::face_for(&child.style, Some(&text));
    let lines: Vec<&str> = text.split('\n').collect();
    let mut widths = Vec::with_capacity(lines.len());
    for line in &lines {
        widths.push(super::sizing::preserved_line(context, child, line, face)?);
    }
    let last = lines.len() - 1;
    Ok(Some(Split {
        head: widths[0],
        middle: widths[1..last].iter().copied().fold(0.0, f64::max),
        tail: (!lines[last].is_empty()).then_some(widths[last]),
    }))
}

/** The children of one line so far, each with the width it takes there. */
#[derive(Default)]
struct Line<'a> {
    nodes: Vec<&'a Node>,
    widths: Vec<f64>,
}

/** The widest line of a container's inline `children` (with their max-content `maxes`): `<br>`
 * and preserved newlines end a line, and a clearing float begins one (Blink resets
 * `floats_inline_size` at clearance). The container's own leading text and `generated` content
 * sit on its first line. */
pub(super) fn longest(
    context: &mut LayoutContext<'_>,
    node: &Node,
    (children, maxes): (&[&Node], &[f64]),
    generated: f64,
) -> EngineResult<f64> {
    let mut widest = Widest {
        longest: 0.0,
        first: true,
        generated,
    };
    let mut line = Line::default();
    for (index, &child) in children.iter().enumerate() {
        let clears = super::sizing::floating(child)
            && child.style.clear != crate::document::style::Clear::None;
        if child.tag == "br" || (clears && index > 0) {
            widest.close(context, node, &mut line)?;
            if child.tag == "br" {
                continue;
            }
        }
        let Some(split) = split(context, child)? else {
            line.nodes.push(child);
            line.widths.push(maxes[index]);
            continue;
        };
        line.nodes.push(child);
        line.widths.push(split.head);
        widest.close(context, node, &mut line)?;
        widest.longest = widest.longest.max(split.middle);
        if let Some(tail) = split.tail {
            line.nodes.push(child);
            line.widths.push(tail);
        }
    }
    widest.close(context, node, &mut line)?;
    Ok(widest.longest)
}

/** The running answer of `longest`: the widest line so far, and what the first line adds. */
struct Widest {
    longest: f64,
    first: bool,
    generated: f64,
}

impl Widest {
    /** Ends `line`: its children's widths, the spaces between them and, on the first line, the
     * container's own text and generated content, then starts an empty one. */
    fn close(
        &mut self,
        context: &mut LayoutContext<'_>,
        node: &Node,
        line: &mut Line<'_>,
    ) -> EngineResult<()> {
        let own = if self.first { self.generated } else { 0.0 };
        let extras = context.inline_extras(node, &line.nodes, self.first)?;
        let width = line.widths.iter().sum::<f64>() + extras + own;
        self.longest = self.longest.max(width);
        self.first = false;
        *line = Line::default();
        Ok(())
    }
}
