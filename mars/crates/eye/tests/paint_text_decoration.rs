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

/** `text-decoration-style` and `text-decoration-thickness` are not inherited (css-text-decor), so a
child of a wavy, thick parent paints its own decoration as solid at the font's thickness. The
underline offset is inherited and still carries to the child. */
#[test]
fn decoration_style_and_thickness_do_not_inherit() {
    let (document, _) = layout(
        "<p id=p><span id=c>c</span></p>",
        "#p{text-decoration:underline wavy 4px;text-underline-offset:3px}\
         #c{text-decoration:underline}",
    );
    let child = &document
        .nodes
        .iter()
        .find(|node| node.html_id == "c")
        .unwrap()
        .style;
    assert_eq!(child.decoration_style, DecorationStyle::Solid);
    assert_eq!(child.decoration_thickness, None);
    assert_eq!(child.underline_offset, Some(3.0));
}

/** The `text-decoration` shorthand drops on an unknown word or a repeated part, so the earlier
value of the same declaration block survives the dropped one (as Blink's parser does). */
#[test]
fn text_decoration_shorthand_drops_unknown_or_repeated_words() {
    for value in [
        "underline bogus",
        "underline underline",
        "wavy dotted",
        "1px 2px",
        "none underline",
        "auto 2px",
    ] {
        let (document, _) = layout(
            "<p id=a>a</p>",
            &format!("#a{{text-decoration:underline wavy 4px;text-decoration:{value}}}"),
        );
        let style = &document
            .nodes
            .iter()
            .find(|node| node.html_id == "a")
            .unwrap()
            .style;
        assert!(style.underline, "{value}");
        assert_eq!(style.decoration_style, DecorationStyle::Wavy, "{value}");
        assert_eq!(style.decoration_thickness, Some(4.0), "{value}");
    }
}

/** A color is a valid word of the shorthand beside its lines, style and thickness. */
#[test]
fn text_decoration_shorthand_accepts_a_color_beside_its_words() {
    let (document, _) = layout(
        "<p id=a>a</p>",
        "#a{text-decoration:underline wavy 2px red}",
    );
    let style = &document
        .nodes
        .iter()
        .find(|node| node.html_id == "a")
        .unwrap()
        .style;
    assert!(style.underline);
    assert_eq!(style.decoration_style, DecorationStyle::Wavy);
    assert_eq!(style.decoration_thickness, Some(2.0));
}
