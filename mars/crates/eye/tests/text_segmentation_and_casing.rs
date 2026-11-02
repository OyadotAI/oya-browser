/** Round-4 CSS Text rules in Eye: CJK min-content per character, `word-break: auto-phrase`,
 * `<wbr>` and glue, dictionary scripts, Tibetan and Hangul breaking, `<nobr>` runs moving
 * whole, language-tailored `text-transform`, collapsible spaces before preserved ones, and
 * `text-indent` in intrinsic widths. */
use mars_eye::{DisplayItem, PaintLine, compute_styles, parse, render};

/** The page width the cases lay out at. */
const VIEWPORT: f64 = 800.0;
/** The base sheet: no margins, a 10px monospace face. */
const BASE: &str = "body{margin:0;font:10px monospace}";

/** Every display item for `markup` styled by `sheet`. */
fn items(markup: &str, sheet: &str) -> Vec<DisplayItem> {
    let document = compute_styles(&parse(markup).unwrap(), &[sheet.into()]).unwrap();
    render(&document, VIEWPORT).unwrap().items
}
/** Every painted run with text, top to bottom then left to right. */
fn runs(markup: &str) -> Vec<PaintLine> {
    let mut lines: Vec<PaintLine> = items(markup, BASE)
        .into_iter()
        .flat_map(|i| i.lines)
        .filter(|l| !l.text.is_empty())
        .collect();
    lines.sort_by(|a, b| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)));
    lines
}
/** The text of each line: the runs at each distinct top, joined. */
fn lines(markup: &str) -> Vec<String> {
    let mut out: Vec<(f64, String)> = Vec::new();
    for run in runs(markup) {
        match out.last_mut().filter(|(y, _)| *y == run.y) {
            Some((_, text)) => text.push_str(&run.text),
            None => out.push((run.y, run.text)),
        }
    }
    out.into_iter().map(|(_, text)| text).collect()
}
/** The width of the `#t` element. */
fn width(markup: &str) -> f64 {
    let document = compute_styles(&parse(markup).unwrap(), &[BASE.into()]).unwrap();
    let local = document
        .nodes
        .iter()
        .find(|n| n.html_id == "t")
        .unwrap()
        .local_id;
    let display = render(&document, VIEWPORT).unwrap();
    display
        .items
        .into_iter()
        .find(|i| i.local_id == local)
        .unwrap()
        .width
}

/** CJK text may break between any two characters, so its min-content is one character. */
#[test]
fn cjk_min_content_is_one_character() {
    assert_eq!(
        width("<div id=t style='width:min-content'>楽しい</div>"),
        10.0
    );
}

/** `auto-phrase` breaks Japanese only between phrases. */
#[test]
fn auto_phrase_breaks_between_phrases() {
    let found = lines(
        "<div lang=ja style='width:min-content;word-break:auto-phrase'>東京へ行きましょう。</div>",
    );
    assert_eq!(found, ["東京へ", "行きましょう。"]);
}

/** A phrase wider than its line moves to a new line, then breaks there as `normal` text. */
#[test]
fn auto_phrase_falls_back_inside_a_too_wide_phrase() {
    let found =
        lines("<div lang=ja style='width:40px;word-break:auto-phrase'>楽しいドライブ。</div>");
    assert_eq!(found, ["楽しい", "ドライ", "ブ。"]);
}

/** `auto-phrase` suppresses soft hyphens while a word fits. */
#[test]
fn auto_phrase_suppresses_soft_hyphens_in_min_content() {
    let phrase =
        width("<div id=t style='width:min-content;word-break:auto-phrase'>con\u{ad}sid</div>");
    let whole = width("<div id=t style='width:min-content'>consid</div>");
    assert_eq!(phrase, whole);
}

/** `<wbr>` is a break opportunity between the runs it separates. */
#[test]
fn wbr_breaks_joined_letters() {
    assert_eq!(
        lines("<div style='width:20px'>abc<wbr>def</div>"),
        ["abc", "def"]
    );
}

/** The `&ZeroWidthSpace;` and `&NoBreak;` references decode. */
#[test]
fn zero_width_references_decode() {
    let found = lines("<div>a&ZeroWidthSpace;b&NoBreak;c</div>");
    assert_eq!(found, ["a\u{200b}b\u{2060}c"]);
}

/** Glue keeps its neighbors even under `word-break: break-all`. */
#[test]
fn break_all_honors_glue() {
    let found = lines("<div style='width:40px;word-break:break-all'>XXXX\u{a0}XXXX</div>");
    assert!(
        found
            .iter()
            .all(|l| !l.starts_with('\u{a0}') && !l.ends_with('\u{a0}')),
        "{found:?}"
    );
}

/** A line breaks after a Tibetan tsheg. */
#[test]
fn tibetan_breaks_after_tsheg() {
    assert!(lines("<div style='width:0'>ལྷ་སའི་</div>").len() > 1);
}

/** Thai words meet where the system dictionary says, with no spaces between them. */
#[cfg(target_os = "macos")]
#[test]
fn thai_breaks_at_dictionary_words() {
    let found = lines("<div style='width:0'>กรุงเทพคือสวยงาม</div>");
    assert_eq!(found, ["กรุงเทพ", "คือ", "สวยงาม"]);
}

/** A Hangul word that overflows fills the line up to a syllable before wrapping. */
#[test]
fn hangul_fills_the_line_between_syllables() {
    let found = lines("<div style='width:50px'>한글이 한글이</div>");
    assert!(
        found[0].chars().count() > "한글이".chars().count(),
        "{found:?}"
    );
}

/** A `nowrap` run moves to the next line whole at the boundary before it. */
#[test]
fn nowrap_run_moves_whole() {
    let found = lines(
        "<div lang=ja style='width:60px'>楽しい<span style='white-space:nowrap'>ドライブ</span></div>",
    );
    assert_eq!(found, ["楽しい", "ドライブ"]);
}

/** Greek uppercase drops accents; Dutch capitalizes "ij" together. */
#[test]
fn text_transform_follows_the_language() {
    let greek = lines("<div lang=el style='text-transform:uppercase'>αύριο</div>");
    assert_eq!(greek, ["ΑΥΡΙΟ"]);
    let dutch = lines("<div lang=nl style='text-transform:capitalize'>ijsland</div>");
    assert_eq!(dutch, ["IJsland"]);
}

/** A collapsible space before preserved white space is kept, not collapsed into it. */
#[test]
fn collapsible_space_before_preserved_space_stays() {
    let found = runs("<div><span>X </span><span style='white-space:pre'> X</span></div>");
    let second = found
        .iter()
        .find(|r| r.text.ends_with('X') && r.x > 0.0)
        .unwrap();
    let first = &found[0];
    let space = width("<span id=t> </span>").max(1.0);
    assert!(second.x >= first.x + first.width + space - 0.5, "{found:?}");
}

/** `text-indent` widens a shrink-to-fit box's first line, and a negative one narrows it. */
#[test]
fn text_indent_counts_in_intrinsic_widths() {
    let plain = width("<div id=t style='float:left'>X</div>");
    let indented = width("<div id=t style='float:left;text-indent:30px'>X</div>");
    assert_eq!(indented, plain + 30.0);
    let hanging = width(
        "<div id=t style='float:left;text-indent:-30px'><span style='display:inline-block;width:50px'></span></div>",
    );
    assert_eq!(hanging, 20.0);
}

/** A fixed `max-width` caps an auto-width table's shrink-to-fit width. */
#[test]
fn table_max_width_caps_shrink_to_fit() {
    let found = width(
        "<table id=t style='max-width:0;word-break:break-word;border-collapse:collapse'><tr><td style='padding:0'>XXXX</table>",
    );
    assert!(found < 20.0, "{found}");
}
