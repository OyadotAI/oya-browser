/** `<meta name=color-scheme>` is the root's color-scheme hint; author CSS on the root wins. */
use mars_eye::{Appearance, Document, RenderContext, compute_styles_with_context, parse};

/** Styles `markup` in dark appearance with the meta content the parser records on the root. */
fn styled(meta: &str, sheet: &str) -> Document {
    let mut document = parse("<p id=a>a</p>").unwrap();
    document
        .formatting
        .html
        .attributes
        .insert("meta color-scheme".into(), meta.into());
    let context = RenderContext {
        appearance: Appearance::Dark,
        ..RenderContext::default()
    };
    compute_styles_with_context(&document, &[sheet.into()], context).unwrap()
}

#[test]
fn meta_color_scheme_sets_the_root_scheme() {
    let document = styled("light dark", "");
    assert_eq!(document.formatting.html.style.used_scheme, Appearance::Dark);
    assert_eq!(document.nodes[0].style.used_scheme, Appearance::Dark);
}

#[test]
fn author_root_color_scheme_beats_the_meta() {
    let document = styled("light dark", "html{color-scheme:light}");
    assert_eq!(
        document.formatting.html.style.used_scheme,
        Appearance::Light
    );
}

#[test]
fn an_invalid_meta_value_is_ignored_quietly() {
    let document = styled("purple", "");
    assert_eq!(
        document.formatting.html.style.used_scheme,
        Appearance::Light
    );
    assert!(
        document
            .diagnostics
            .iter()
            .all(|d| !d.contains("color-scheme"))
    );
}
