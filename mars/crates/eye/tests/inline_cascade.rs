/** Cascade rules from the inline builder's pass: leading `@import` rules are found for the
 * runtime to fetch, media queries read absolute units and unknown media types, `ex` and `cap`
 * measure the face, `attr()` substitutes attributes, and `all` resets every property. */
use mars_eye::{compute_styles, css_imports, parse, render};

/** The color of each div in `markup` styled by `sheet`. */
fn colors(markup: &str, sheet: &str) -> Vec<Option<String>> {
    let document = compute_styles(&parse(markup).unwrap(), &[sheet.to_string()]).unwrap();
    let items = render(&document, 800.0).unwrap().items;
    items
        .iter()
        .filter(|item| item.tag == "div")
        .map(|item| item.color.clone())
        .collect()
}

/** Imports count before other rules (after `@charset`, `@layer` statements and dropped rules),
 * with their media; a valid style rule ends them, and a stray token swallows the next one. */
#[test]
fn leading_imports_are_found_with_their_media() {
    let found = css_imports(
        "@charset \"utf-8\";\n@media;\n# { color: red }\n@import url(\"a.css\") print;\n@import 'b.css';\np { color: green }\n@import 'c.css';",
    );
    assert_eq!(
        found,
        vec![
            ("a.css".to_string(), "print".to_string()),
            ("b.css".to_string(), String::new())
        ]
    );
    assert!(css_imports("# :unknown\n@import 'red.css';\n.x { color: red }").is_empty());
    let commented =
        css_imports("@import 'g.css' (min-width: 1px) /* note */ and (max-width: 9in);");
    assert_eq!(commented[0].1, "(min-width: 1px) and (max-width: 9in)");
}

/** `attr()` substitutes the element's attribute by its type; an invalid or missing one takes
 * the fallback, and one with neither is unset. */
#[test]
fn attr_substitutes_typed_attributes_with_fallbacks() {
    let sheet = "div { color: attr(data-c type(<color>), red) }";
    let found = colors(
        "<div data-c=green>a</div><div data-c=qqq>b</div><div>c</div>",
        sheet,
    );
    assert_eq!(
        found,
        vec![
            Some("green".to_string()),
            Some("red".to_string()),
            Some("red".to_string())
        ]
    );
    let document = compute_styles(
        &parse("<div data-w=40 style='width: attr(data-w px)'></div>").unwrap(),
        &[],
    )
    .unwrap();
    assert_eq!(render(&document, 800.0).unwrap().items[0].width, 40.0);
}

/** `all` sets every property to a CSS-wide keyword: `initial` drops the authored color, and
 * `revert-layer` rolls each property back to the previous layer's value. */
#[test]
fn all_resets_every_property() {
    let found = colors(
        "<div class=a>a</div><div class=b>b</div>",
        ".a { color: red } .a { all: initial } @layer { .b { color: green } } @layer { .b { color: red } .b { all: revert-layer } }",
    );
    assert_eq!(found, vec![None, Some("green".to_string())]);
}

/** `ex` is the element's x-height and `cap` its cap height, each a fraction of the em, and a
 * value merely containing the letters (`flex`) is not taken for the unit. */
#[test]
fn ex_and_cap_measure_the_face() {
    let document = compute_styles(
        &parse("<div id=a style='font-size:20px;height:10ex;width:10cap'></div>").unwrap(),
        &[],
    )
    .unwrap();
    let item = &render(&document, 800.0).unwrap().items[0];
    assert!((60.0..=180.0).contains(&item.height), "{}", item.height);
    assert!((100.0..=190.0).contains(&item.width), "{}", item.width);
}

/** Media features take absolute units, and an unknown media type is false, not undecided. */
#[test]
fn media_lengths_take_absolute_units_and_unknown_types_fail() {
    let sheet = "@media (max-width: 40000in) { .a { color: green } }\n@media nonsense, all { .b { color: green } }\n@media nonsense { .c { color: green } }";
    let found = colors(
        "<div class=a>a</div><div class=b>b</div><div class=c>c</div>",
        sheet,
    );
    assert_eq!(
        found,
        vec![Some("green".to_string()), Some("green".to_string()), None]
    );
}
