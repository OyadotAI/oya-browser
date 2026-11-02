/** Page metadata the style engine reads: `<meta name=color-scheme>` sets the root's scheme hint. */
use mars_eye::{ColorScheme, compute_styles, parse};

/** A color-scheme meta reaches the root's computed color-scheme, in its order. */
#[test]
fn color_scheme_meta_sets_the_root_scheme() {
    let document =
        parse("<head><meta name=Color-Scheme content='dark light'></head><p>x</p>").unwrap();
    let styled = compute_styles(&document, &[]).unwrap();
    assert_eq!(
        styled.formatting.html.style.color_scheme,
        ColorScheme::DarkLight
    );
}

/** Inserted markup never sets document metadata. */
#[test]
fn inserted_color_scheme_meta_is_ignored() {
    let fragment =
        mars_eye::parse_fragment("body", "<meta name=color-scheme content=dark><p>x</p>", 5)
            .unwrap();
    assert_eq!(fragment.nodes.len(), 1);
}
