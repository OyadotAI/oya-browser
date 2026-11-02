/** CSS Text rules in Eye's inline layout: `word-spacing` reaching painters, `text-align: justify`
 * with `text-align-last` and `text-justify`, `match-parent`, ideographic spaces hanging at a
 * line's end, and glue that forbids a break beside an atomic inline. */
use mars_eye::{DisplayItem, PaintLine, compute_styles, parse, render};

/** The page width the cases lay out at. */
const VIEWPORT: f64 = 800.0;
/** Layout positions agree to this much. */
const EPSILON: f64 = 0.01;

/** Every display item for `markup` styled by `sheet`, with the local id of `#t`. */
fn layout(markup: &str, sheet: &str) -> (Vec<DisplayItem>, usize) {
    let document = compute_styles(&parse(markup).unwrap(), &[sheet.into()]).unwrap();
    let local = document
        .nodes
        .iter()
        .find(|n| n.html_id == "t")
        .unwrap()
        .local_id;
    (render(&document, VIEWPORT).unwrap().items, local)
}
/** The `#t` element's display item. */
fn target(markup: &str, sheet: &str) -> DisplayItem {
    let (items, local) = layout(markup, sheet);
    items.into_iter().find(|i| i.local_id == local).unwrap()
}
/** `#t`'s painted runs sorted top to bottom, then left to right. */
fn runs(markup: &str, sheet: &str) -> Vec<PaintLine> {
    let mut lines = target(markup, sheet).lines;
    lines.sort_by(|a, b| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)));
    lines
}
/** The distinct line tops of `runs`. */
fn tops(runs: &[PaintLine]) -> Vec<f64> {
    let mut tops: Vec<f64> = runs.iter().map(|r| r.y).collect();
    tops.dedup();
    tops
}
/** The right edge of the last run on the line at `top`. */
fn line_end(runs: &[PaintLine], top: f64) -> f64 {
    runs.iter()
        .filter(|r| r.y == top)
        .map(|r| r.x + r.width)
        .fold(0.0, f64::max)
}

/** The base sheet: no margins, a 16px serif face. */
const BASE: &str = "body{margin:0;font:16px serif}";

/** `word-spacing` reaches the display item, so painters space glyphs as layout did. */
#[test]
fn word_spacing_reaches_the_display_item() {
    let item = target("<div id=t style='word-spacing:12px'>a b</div>", BASE);
    assert_eq!(item.word_spacing, 12.0);
}

/** A justified line that ended at a soft wrap fills its line box; the last line does not. */
#[test]
fn justify_fills_wrapped_lines_but_not_the_last() {
    let sheet = format!("{BASE}#t{{width:200px;text-align:justify}}");
    let found = runs(
        "<div id=t>aaa bbb ccc ddd eee fff ggg hhh iii jjj kkk lll mmm</div>",
        &sheet,
    );
    let tops = tops(&found);
    assert!(tops.len() > 1, "{found:?}");
    assert!(
        (line_end(&found, tops[0]) - 200.0).abs() < EPSILON,
        "{found:?}"
    );
    assert!(line_end(&found, tops[tops.len() - 1]) < 200.0 - 1.0);
}

/** A justified run is split after each space, the pieces tiling the line left to right. */
#[test]
fn justify_splits_runs_at_spaces() {
    let sheet = format!("{BASE}#t{{width:200px;text-align:justify}}");
    let found = runs(
        "<div id=t>aaa bbb ccc ddd eee fff ggg hhh iii jjj</div>",
        &sheet,
    );
    let first: Vec<&PaintLine> = found.iter().filter(|r| r.y == found[0].y).collect();
    assert!(first.len() > 2, "{found:?}");
    for pair in first.windows(2) {
        assert!(
            (pair[0].x + pair[0].width - pair[1].x).abs() < EPSILON,
            "{pair:?}"
        );
    }
}

/** `text-align-last: justify` spreads the last line too. */
#[test]
fn text_align_last_justify_spreads_the_last_line() {
    let sheet = format!("{BASE}#t{{width:300px;text-align-last:justify}}");
    let found = runs("<div id=t>aaa bbb ccc</div>", &sheet);
    assert!(
        (line_end(&found, found[0].y) - 300.0).abs() < EPSILON,
        "{found:?}"
    );
}

/** `text-align-last` other than `auto` aligns the last line its own way. */
#[test]
fn text_align_last_aligns_the_last_line() {
    let sheet = format!("{BASE}#t{{width:300px;text-align-last:right}}");
    let found = runs("<div id=t>aaa bbb ccc</div>", &sheet);
    assert!(
        (line_end(&found, found[0].y) - 300.0).abs() < EPSILON,
        "{found:?}"
    );
}

/** `text-justify: none` turns justification off: the line aligns at its start. */
#[test]
fn text_justify_none_aligns_at_start() {
    let sheet = format!("{BASE}#t{{width:300px;text-align-last:justify;text-justify:none}}");
    let found = runs("<div id=t>aaa bbb ccc</div>", &sheet);
    assert_eq!(found[0].x, 0.0);
    assert!(line_end(&found, found[0].y) < 300.0 - 1.0);
}

/** `inter-character` spreads between every character, not only at spaces. */
#[test]
fn inter_character_spreads_between_letters() {
    let sheet =
        format!("{BASE}#t{{width:300px;text-align-last:justify;text-justify:inter-character}}");
    let found = runs("<div id=t>abcd</div>", &sheet);
    assert_eq!(found.len(), 4, "{found:?}");
    assert!(
        (line_end(&found, found[0].y) - 300.0).abs() < EPSILON,
        "{found:?}"
    );
}

/** Only what follows a line's last tab spreads: a tab advances to a fixed stop. */
#[test]
fn justify_leaves_text_before_a_tab_alone() {
    let sheet = format!("{BASE}#t{{width:400px;white-space:pre-wrap;text-align-last:justify}}");
    let found = runs("<div id=t>a b\tc d</div>", &sheet);
    let head = found.iter().find(|r| r.text.starts_with('a')).unwrap();
    assert_eq!(head.text, "a b", "{found:?}");
    assert!(
        (line_end(&found, found[0].y) - 400.0).abs() < EPSILON,
        "{found:?}"
    );
}

/** `match-parent` resolves the parent's `start` against the parent's direction, so a
 * right-to-left child of a left-to-right parent still aligns left. */
#[test]
fn match_parent_resolves_against_the_parents_direction() {
    let found = runs(
        "<div style='text-align:start'><div id=t dir=rtl style='width:300px;text-align:match-parent'>abc</div></div>",
        BASE,
    );
    assert_eq!(found[0].x, 0.0);
}

/** Ideographic spaces ending a run hang past the line's end instead of wrapping its letter. */
#[test]
fn trailing_ideographic_spaces_hang() {
    let sheet = "body{margin:0;font:20px monospace}#t{width:60px}";
    let found = runs("<div id=t>X\u{3000}<span>X\u{3000}</span></div>", sheet);
    assert_eq!(found.len(), 1, "{found:?}");
    let (items, _) = layout("<div id=t>X\u{3000}<span>X\u{3000}</span></div>", sheet);
    let tops: Vec<f64> = items
        .iter()
        .flat_map(|i| i.lines.iter())
        .filter(|l| !l.text.is_empty())
        .map(|l| l.y)
        .collect();
    assert!(tops.iter().all(|y| *y == tops[0]), "{tops:?}");
}

/** A text node of only ideographic spaces is text, not droppable HTML white space. */
#[test]
fn an_ideographic_space_only_text_node_is_kept() {
    let (items, _) = layout("<div id=t>a<br>\u{3000}\u{3000}</div>", BASE);
    let found: Vec<&PaintLine> = items.iter().flat_map(|i| i.lines.iter()).collect();
    assert!(
        found.iter().any(|r| r.text.contains('\u{3000}')),
        "{found:?}"
    );
}

/** Glue before an atomic inline (a narrow no-break space) keeps it on the text's line. */
#[test]
fn glue_keeps_an_atomic_inline_on_the_line() {
    let markup = "<div style='width:20px'>A\u{202f}<span id=t style='display:inline-block;width:20px;height:10px'></span></div>";
    let (items, local) = layout(markup, BASE);
    let atomic = items.iter().find(|i| i.local_id == local).unwrap();
    let text = items
        .iter()
        .flat_map(|i| i.lines.iter())
        .find(|l| l.text.starts_with('A'));
    let text = text.unwrap();
    assert!(
        (atomic.x - (text.x + text.width)).abs() < EPSILON,
        "{text:?} {}",
        atomic.x
    );
}

/** A shrink-to-fit box counts `word-spacing` in its max-content width, so its words stay on
 * one line. */
#[test]
fn max_content_counts_word_spacing() {
    let item = target(
        "<div><span id=t style='display:inline-block;word-spacing:100px'>a b</span></div>",
        BASE,
    );
    assert_eq!(item.lines.len(), 1, "{:?}", item.lines);
    assert!(item.width > 100.0, "{}", item.width);
}

/** A trailing ideographic space hangs, so it adds nothing to a box's min-content width. */
#[test]
fn min_content_leaves_out_a_hanging_ideographic_space() {
    let item = target(
        "<div style='width:0'><div id=t style='width:min-content'>\u{3000}</div></div>",
        BASE,
    );
    assert_eq!(item.width, 0.0);
}
