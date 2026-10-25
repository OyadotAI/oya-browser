/**
 * Style ports from Blink: `:nth-child(An+B of S)` tokenization of `of`, namespace-prefixed type
 * selectors without `@namespace`, and font-relative `ch`/`ic` measured from the element's face.
 */
use mars_eye::platform::{Face, Family, measure};
use mars_eye::{Document, RenderContext, compute_styles, compute_styles_with_context, parse};

/** The color of `#t` after styling `markup` with `sheet`. */
fn color(markup: &str, sheet: &str) -> Option<String> {
    let document: Document = compute_styles(&parse(markup).unwrap(), &[sheet.into()]).unwrap();
    let node = document.nodes.iter().find(|n| n.html_id == "t").unwrap();
    node.style.color.clone()
}
/** Whether `selector` matches the third `p`, `#t`, of `.a`, plain, `.a` siblings. */
fn hit(selector: &str) -> bool {
    let markup = "<div><p class=a></p><p></p><p id=t class=a></p></div>";
    color(markup, &format!("{selector} {{ color: green }}")).as_deref() == Some("green")
}
/** `#t`'s computed width for `width: value` with the given font shorthand. */
fn width(font: &str, value: &str) -> f64 {
    let sheet = format!("#t {{ font: {font}; width: {value} }}");
    let context = RenderContext::default();
    let document =
        compute_styles_with_context(&parse("<div id=t>x</div>").unwrap(), &[sheet], context)
            .unwrap();
    let node = document.nodes.iter().find(|n| n.html_id == "t").unwrap();
    node.style.width.unwrap()
}

#[test]
fn nth_child_of_needs_no_space_after_of() {
    assert!(hit(":nth-child(2 of.a)"));
    assert!(hit(":nth-child(2 of/* c */.a)"));
    assert!(hit(":nth-child(2 of[class])"));
}

#[test]
fn nth_child_of_keyword_must_be_its_own_token() {
    assert!(!hit("p:nth-child(n + 1of .a)"));
    assert!(!hit("p:nth-child(2 of)"));
    assert!(!hit("p:nth-child(2 ofx .a)"));
}

#[test]
fn any_namespace_prefix_matches_like_no_prefix() {
    assert!(hit("*|p:nth-child(3)"));
    assert!(hit("p:nth-child(odd of *|*)"));
}

#[test]
fn no_namespace_prefix_never_matches_html_elements() {
    assert!(!hit("|p"));
}

#[test]
fn ch_is_the_advance_of_zero_in_the_elements_face() {
    let face = Face {
        family: Family::Monospace,
        ..Face::system(20.0, false)
    };
    let zero = measure("0", face);
    assert!((width("20px monospace", "1ch") - zero).abs() < 1e-6);
}

#[test]
fn ic_falls_back_to_one_em_without_a_page_font() {
    assert_eq!(width("20px serif", "2ic"), 40.0);
}

#[test]
fn a_namespace_declaration_drops_prefixed_selectors_eye_cannot_resolve() {
    let markup = "<div><p class=a></p><p></p><p id=t class=a></p></div>";
    let sheet = "@namespace url(http://www.w3.org/2000/svg); *|p { color: green }";
    assert_eq!(color(markup, sheet), None);
}
