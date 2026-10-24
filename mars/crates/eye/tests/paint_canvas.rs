/** The canvas a painter fills before any box: Chromium's rule that only an opted-in page goes dark. */
use mars_eye::{
    Appearance, DisplayList, RenderContext, compute_styles_with_context, parse, render_with_context,
};

/** The display list of markup under a native appearance. */
fn paint(markup: &str, appearance: Appearance) -> DisplayList {
    let context = RenderContext {
        appearance,
        ..RenderContext::default()
    };
    let document = compute_styles_with_context(&parse(markup).unwrap(), &[], context).unwrap();
    render_with_context(&document, context).unwrap()
}

/** A page with no color-scheme paints a white canvas with black text even when the OS is dark. */
#[test]
fn page_without_scheme_stays_light_under_dark_os() {
    let display = paint("<p>Plain</p>", Appearance::Dark);
    assert_eq!(display.canvas_background.as_deref(), Some("#ffffff"));
    assert_eq!(display.canvas_foreground.as_deref(), Some("#000000"));
    assert_eq!(display.canvas_scheme, Appearance::Light);
}

/** A root that offers both schemes follows the OS into dark, canvas and controls alike. */
#[test]
fn page_offering_dark_follows_dark_os() {
    let markup = "<html style='color-scheme:light dark'><p>Both</p></html>";
    let display = paint(markup, Appearance::Dark);
    assert_eq!(display.canvas_background.as_deref(), Some("#000000"));
    assert_eq!(display.canvas_foreground.as_deref(), Some("#ffffff"));
    assert_eq!(display.canvas_scheme, Appearance::Dark);
    let light = paint(markup, Appearance::Light);
    assert_eq!(light.canvas_background.as_deref(), Some("#ffffff"));
}

/** An author's own root background still wins over the scheme's canvas. */
#[test]
fn author_background_wins_over_scheme_canvas() {
    let display = paint(
        "<html style='background:#123456'><p>Own</p></html>",
        Appearance::Dark,
    );
    assert_eq!(display.canvas_background.as_deref(), Some("#123456"));
}
