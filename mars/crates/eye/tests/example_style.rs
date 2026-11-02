/** Cascade rules example.com's inserted style sheet relies on: `p+p`, opacity, negative margins, direction. */
mod support;
use mars_eye::{Appearance, Document, Node, compute_styles, parse, render};
use support::{applied, display, styled};

/** The node carrying one `lang` value. */
fn by_lang<'a>(document: &'a Document, lang: &str) -> &'a Node {
    document
        .nodes
        .iter()
        .find(|node| {
            node.attributes
                .get("lang")
                .is_some_and(|value| value == lang)
        })
        .unwrap()
}

/** `p+p` matches paragraphs two to six but not the first one, whose previous sibling is the svg. */
#[test]
fn adjacent_sibling_matches_every_paragraph_but_the_first() {
    let document = styled(&applied("fr"), Appearance::Light);
    let first = by_lang(&document, "fr");
    assert_eq!(first.style.font_size, 16.0);
    assert_eq!(first.style.opacity, 1.0);
    for lang in ["en", "ar", "zh", "ru", "es"] {
        let paragraph = by_lang(&document, lang);
        assert_eq!(
            paragraph.style.font_size,
            0.875 * first.style.font_size,
            "{lang}"
        );
        assert_eq!(paragraph.style.opacity, 0.6, "{lang}");
    }
}

/** Text nodes are not elements: `+` skips them like browsers, and a different element breaks the chain. */
#[test]
fn adjacent_sibling_skips_text_nodes_but_not_other_elements() {
    let document =
        parse("<div><p>a</p>loose<p id=b>b</p><span>s</span><p id=c>c</p></div>").unwrap();
    let document = compute_styles(&document, &["p+p{opacity:.5}".into()]).unwrap();
    let opacity = |id: &str| {
        document
            .nodes
            .iter()
            .find(|node| node.html_id == id)
            .unwrap()
            .style
            .opacity
    };
    assert_eq!(opacity("b"), 0.5);
    assert_eq!(opacity("c"), 1.0);
}

/** `+` combines with descendant combinators and counts both compounds' specificity. */
#[test]
fn adjacent_sibling_mixes_with_descendants() {
    let document = parse("<div class=x><p>a</p><p id=b>b</p></div><p>c</p><p id=d>d</p>").unwrap();
    let sheets = [".x p+p{opacity:.5} p+p{opacity:.25}".to_string()];
    let document = compute_styles(&document, &sheets).unwrap();
    let opacity = |id: &str| {
        document
            .nodes
            .iter()
            .find(|node| node.html_id == id)
            .unwrap()
            .style
            .opacity
    };
    assert_eq!(opacity("b"), 0.5);
    assert_eq!(opacity("d"), 0.25);
}

/** Malformed combinator chains drop their rule with a diagnostic rather than guessing. */
#[test]
fn malformed_adjacent_selectors_are_dropped() {
    let document = parse("<p>x</p>").unwrap();
    for sheet in ["+p{opacity:.5}", "p+{opacity:.5}", "p++p{opacity:.5}"] {
        let styled = compute_styles(&document, &[sheet.into()]).unwrap();
        assert_eq!(styled.nodes[0].style.opacity, 1.0, "{sheet}");
        assert!(
            styled.diagnostics.iter().any(|d| d.contains("selector")),
            "{sheet}"
        );
    }
}

/** The svg is a 44x44 centred block, pulled up by -2.75 times its font size, at opacity 0.55. */
#[test]
fn icon_is_centred_with_negative_em_margin_and_opacity() {
    let (document, display) = display("en", Appearance::Light);
    let svg = document
        .nodes
        .iter()
        .find(|node| node.tag == "svg")
        .unwrap();
    assert_eq!(svg.style.margin[0], -2.75 * svg.style.font_size);
    assert_eq!(svg.style.opacity, 0.55);
    let item = display.items.iter().find(|item| item.tag == "svg").unwrap();
    let paragraph = display.items.iter().find(|item| item.tag == "p").unwrap();
    assert_eq!((item.width, item.height), (44.0, 44.0));
    assert_eq!(
        item.x + item.width / 2.0,
        paragraph.x + paragraph.width / 2.0
    );
    assert_eq!(item.opacity, 0.55);
    assert_eq!(item.role, "image");
    assert!(paragraph.y > item.y + item.height);
}

/** The Arabic paragraph is right-to-left and still inherits the body's centred alignment. */
#[test]
fn arabic_paragraph_is_rtl_and_inherits_center() {
    let (document, display) = display("en", Appearance::Light);
    let arabic = by_lang(&document, "ar");
    assert!(arabic.style.rtl);
    assert_eq!(arabic.style.align, "center");
    let item = display
        .items
        .iter()
        .find(|item| item.local_id == arabic.local_id)
        .unwrap();
    assert!(item.rtl);
    for line in &item.lines {
        let left = line.x - item.x;
        let right = item.x + item.width - (line.x + line.width);
        assert!((left - right).abs() < 0.01);
    }
}

/** Start and end follow the base direction; explicit sides never flip. */
#[test]
fn start_and_end_alignment_follow_direction() {
    let document = parse(
        "<p id=a dir=rtl>short</p><p id=b style='text-align:end'>short</p><p id=c dir=rtl style='text-align:left'>short</p>",
    )
    .unwrap();
    let document = compute_styles(&document, &[]).unwrap();
    let display = render(&document, 640.0).unwrap();
    let line = |id: &str| {
        let local = document
            .nodes
            .iter()
            .find(|n| n.html_id == id)
            .unwrap()
            .local_id;
        let item = display.items.iter().find(|i| i.local_id == local).unwrap();
        (
            item.x,
            item.x + item.width,
            item.lines[0].x,
            item.lines[0].x + item.lines[0].width,
        )
    };
    for id in ["a", "b"] {
        let (_, right, _, end) = line(id);
        assert!((right - end).abs() < 0.01, "{id} is right-aligned");
    }
    let (left, _, start, _) = line("c");
    assert_eq!(left, start);
}

/** Negative em margins resolve against the element's font; extreme magnitudes clamp and leave agent reach. */
#[test]
fn negative_margins_resolve_and_stay_bounded() {
    let document = parse("<p>x</p>").unwrap();
    let styled = compute_styles(&document, &["p{margin-top:-1.5em}".into()]).unwrap();
    assert_eq!(styled.nodes[0].style.margin[0], -24.0);
    let styled = compute_styles(&document, &["p{margin-top:-99999px}".into()]).unwrap();
    let style = &styled.nodes[0].style;
    assert_eq!(style.margin[0], -mars_eye::constants::CSS_MAX_SCALAR);
    assert!(!style.interactive);
}

/** Opacity clamps like CSS, is not inherited, and forged out-of-range values fail validation. */
#[test]
fn opacity_is_bounded_and_not_inherited() {
    let document = parse("<div><p>x</p></div>").unwrap();
    let styled = compute_styles(&document, &["div{opacity:2}".into()]).unwrap();
    assert_eq!(styled.nodes[0].style.opacity, 1.0);
    let styled = compute_styles(&document, &["div{opacity:.5}".into()]).unwrap();
    assert_eq!(styled.nodes[1].style.opacity, 1.0);
    let display = render(&styled, 640.0).unwrap();
    assert!(display.items.iter().all(|item| item.opacity == 0.5));
    let mut forged = styled;
    forged.nodes[0].style.opacity = 1.5;
    assert_eq!(render(&forged, 640.0).unwrap_err().code, "invalid_request");
}

/** One cascade pass over `count` siblings against a full sheet of 59-compound `*+` chains. */
fn sibling_pass(count: usize) -> (Document, std::time::Duration) {
    let document = parse(&"<i></i>".repeat(count)).unwrap();
    let sheet = format!("{}*{{color:red}}", "*+".repeat(58)).repeat(128);
    assert!(sheet.len() <= mars_eye::constants::STYLE_BYTES);
    let started = std::time::Instant::now();
    let document = compute_styles(&document, &[sheet]).unwrap();
    (document, started.elapsed())
}

/** Hostile `+` chains stay linear in sibling count: each step is an index lookup, not a rescan. */
#[test]
fn adjacent_sibling_chains_scale_linearly_with_siblings() {
    let (_, half) = sibling_pass(250);
    let (document, full) = sibling_pass(500);
    let elements: Vec<_> = document.nodes.iter().filter(|n| n.tag == "i").collect();
    assert_eq!(elements[57].style.color, None);
    assert_eq!(elements[58].style.color.as_deref(), Some("red"));
    assert!(full < half * 3, "{half:?} -> {full:?} is superlinear");
}
