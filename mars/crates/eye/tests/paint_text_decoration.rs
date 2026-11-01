/** Text decoration lines carry Blink's style, thickness and underline offset into the display list. */
use mars_eye::{
    DecorationLine, DecorationStyle, DisplayItem, DisplayList, Document, StyleSource,
    compute_styles, parse, render,
};

/** Lays out `markup` with `sheet` at 800px. */
fn layout(markup: &str, sheet: &str) -> (Document, DisplayList) {
    let mut parsed = parse(markup).unwrap();
    let mut sheets: Vec<String> = parsed
        .styles
        .iter()
        .filter_map(|source| match source {
            StyleSource::Inline(css) => Some(css.clone()),
            _ => None,
        })
        .collect();
    sheets.push(sheet.to_string());
    parsed.styles.clear();
    let document = compute_styles(&parsed, &sheets).unwrap();
    let display = render(&document, 800.0).unwrap();
    (document, display)
}

/** The first display item painted for the element with `id`. */
fn item<'a>(document: &Document, display: &'a DisplayList, id: &str) -> &'a DisplayItem {
    let local = document
        .nodes
        .iter()
        .find(|node| node.html_id == id)
        .unwrap()
        .local_id;
    display
        .items
        .iter()
        .find(|item| item.local_id == local)
        .unwrap()
}

/** The decoration line of the text in the element with `id`, which must carry one. */
fn line(sheet: &str) -> DecorationLine {
    let (document, display) = layout("<p id=t>decorated</p>", sheet);
    item(&document, &display, "t").decoration.unwrap()
}

/** Plain text keeps the solid default, so painters draw the line they always did. */
#[test]
fn default_line_is_not_carried() {
    let (document, display) = layout("<p id=t>plain</p>", "");
    assert!(item(&document, &display, "t").decoration.is_none());
}

/** Each `text-decoration-style` keyword reaches the display list. */
#[test]
fn style_keywords_reach_the_display_list() {
    for (keyword, style) in [
        ("double", DecorationStyle::Double),
        ("dotted", DecorationStyle::Dotted),
        ("dashed", DecorationStyle::Dashed),
        ("wavy", DecorationStyle::Wavy),
    ] {
        let sheet = format!("p{{text-decoration:underline;text-decoration-style:{keyword}}}");
        assert_eq!(line(&sheet).style, style, "{keyword}");
    }
}

/** `text-decoration-thickness` and `text-underline-offset` carry their lengths, and `auto` leaves them unset. */
#[test]
fn thickness_and_offset_carry_their_lengths() {
    let sheet =
        "p{text-decoration:underline;text-decoration-thickness:3px;text-underline-offset:-2px}";
    let carried = line(sheet);
    assert_eq!(carried.thickness, Some(3.0));
    assert_eq!(carried.offset, Some(-2.0));
    let auto = line(
        "p{text-decoration:underline wavy;text-decoration-thickness:auto;text-underline-offset:auto}",
    );
    assert_eq!((auto.thickness, auto.offset), (None, None));
}

/** The `text-decoration` shorthand sets the style and resets an earlier thickness it does not name. */
#[test]
fn shorthand_sets_style_and_resets_thickness() {
    let sheet = "p{text-decoration-thickness:4px}p{text-decoration:underline dotted}";
    let shorthand = line(sheet);
    assert_eq!(shorthand.style, DecorationStyle::Dotted);
    assert_eq!(shorthand.thickness, None);
}

/** The three decoration longhands and the shorthand are supported, so none is a diagnostic. */
#[test]
fn decoration_properties_are_no_longer_diagnostics() {
    let (document, _) = layout(
        "<p>x</p>",
        "p{text-decoration-style:wavy;text-decoration-thickness:2px;text-underline-offset:3px}",
    );
    assert!(
        !document
            .diagnostics
            .iter()
            .any(|message| message.contains("text-underline-offset")
                || message.contains("text-decoration")),
        "{:?}",
        document.diagnostics
    );
}

/** A decoration thickness of zero or below a visible line is raised to the minimum, never painted empty. */
#[test]
fn thin_thickness_is_raised_to_the_minimum_line() {
    assert_eq!(
        line("p{text-decoration:underline;text-decoration-thickness:0.01px}").thickness,
        Some(0.5)
    );
    assert_eq!(
        line("p{text-decoration:underline wavy;text-decoration-thickness:0px}").thickness,
        Some(0.5)
    );
}
