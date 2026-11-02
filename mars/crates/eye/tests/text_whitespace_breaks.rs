/** UAX #14 punctuation classes between ideographs (OP, CL, CP, EX, IS, NS) and the CSS Text 4
white-space longhands (`white-space-collapse`, `text-wrap-mode`) folded into `white-space`. */
use mars_eye::{DisplayItem, compute_styles, parse, render};

/** The painted lines of `#t` after laying out `markup` at 800px. */
fn lines(markup: &str) -> Vec<String> {
    let markup = format!("<body style='margin:0'>{markup}");
    let document = compute_styles(&parse(&markup).unwrap(), &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    let local = document
        .nodes
        .iter()
        .find(|n| n.html_id == "t")
        .unwrap()
        .local_id;
    let item: &DisplayItem = display.items.iter().find(|i| i.local_id == local).unwrap();
    item.lines.iter().map(|line| line.text.clone()).collect()
}

/** A box three ideographs wide at 20px. */
const NARROW: &str = "font-size:20px;width:62px";

/** LB14: opening punctuation (here a Tibetan mark and a low-9 quote) never ends a line. */
#[test]
fn opening_punctuation_never_ends_a_line() {
    for open in ['\u{0f3a}', '\u{201e}', '\u{2768}', '\u{29fc}'] {
        let found = lines(&format!("<div id=t style='{NARROW}'>中中{open}文</div>"));
        assert!(
            found.iter().all(|line| !line.ends_with(open)),
            "{open:?}: {found:?}"
        );
    }
}

/** LB13 and LB16: closing punctuation, exclamations and nonstarters never start a line. */
#[test]
fn closing_punctuation_and_nonstarters_never_start_a_line() {
    for close in [
        '\u{0f3b}', '\u{2769}', '\u{2998}', '\u{203c}', '\u{30fd}', '\u{ff9e}',
    ] {
        let found = lines(&format!("<div id=t style='{NARROW}'>中中中{close}文</div>"));
        assert!(
            found.iter().skip(1).all(|line| !line.starts_with(close)),
            "{close:?}: {found:?}"
        );
    }
}

/** Ideographs without punctuation between them still break anywhere. */
#[test]
fn ideographs_still_break_between_each_other() {
    let found = lines(&format!("<div id=t style='{NARROW}'>中中中中文</div>"));
    assert!(found.len() > 1, "{found:?}");
}

/** `text-wrap-mode: nowrap` stops wrapping like `white-space: nowrap`. */
#[test]
fn text_wrap_mode_nowrap_keeps_one_line() {
    let found = lines("<div id=t style='width:40px;text-wrap-mode:nowrap'>aa bb cc</div>");
    assert_eq!(found, ["aa bb cc"]);
}

/** `white-space-collapse: preserve-breaks` keeps newlines as forced breaks. */
#[test]
fn preserve_breaks_keeps_newlines() {
    let found = lines("<div id=t style='white-space-collapse:preserve-breaks'>aa\nbb</div>");
    assert_eq!(found, ["aa", "bb"]);
}

/** The two-value `white-space` shorthand reads like the CSS 2 keyword it stands for. */
#[test]
fn two_value_white_space_shorthand() {
    let found = lines("<div id=t style='width:40px;white-space:collapse nowrap'>aa bb cc</div>");
    assert_eq!(found, ["aa bb cc"]);
}

/** UAX #14 LB31: a closing mark may end a line before an opening one (`）（`). */
#[test]
fn closing_then_opening_punctuation_may_break() {
    let found = lines(&format!(
        "<div id=t style='{NARROW}'>国国\u{ff09}\u{ff08}国</div>"
    ));
    assert_eq!(found, ["国国\u{ff09}", "\u{ff08}国"]);
}

/** UAX #29: `word-break: break-all` never splits a grapheme cluster (a ZWJ emoji sequence, a
flag, a base and its combining mark). */
#[test]
fn break_all_keeps_grapheme_clusters_whole() {
    for cluster in [
        "\u{1f468}\u{200d}\u{1f4bb}",
        "\u{1f1ef}\u{1f1f5}",
        "e\u{301}",
        "\u{1f44d}\u{1f3fd}",
    ] {
        let found = lines(&format!(
            "<div id=t style='width:1px;word-break:break-all'>{cluster}</div>"
        ));
        assert_eq!(found, [cluster], "{cluster:?}");
    }
}
