/** Static CSS is verified through owned parse, cascade and render surfaces with unchanged source text. */
use mars::document::{
    Appearance, ColorScheme, Document, LineHeight, Position, RenderContext, StyleSource,
    compute_styles_with_context, parse, render_with_context,
};

/** The retained source includes its required script rather than manufacturing a runtime-compatible page. */
const ORIGINAL: &str = include_str!("../../../fixtures/reading/example.html");

/** Chromium's UA body margin, which insets every unstyled page. */
const BODY_MARGIN: f64 = 8.0;
/** The UA stylesheet's 1em paragraph block margin at the 16px initial font size, as in Chromium. */
const P_MARGIN: f64 = 16.0;

/** All test contexts use a real page viewport rather than shell window dimensions. */
fn context(width: f64, height: f64, appearance: Appearance) -> RenderContext {
    RenderContext {
        width,
        height,
        appearance,
    }
}

/** Inline style source is retained by the parser and then passed through the same cascade entry point. */
fn computed(source: &str, context: RenderContext) -> Document {
    let document = parse(source).unwrap();
    let sheets = document
        .styles
        .iter()
        .filter_map(|source| match source {
            StyleSource::Inline(sheet) => Some(sheet.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    compute_styles_with_context(&document, &sheets, context).unwrap()
}

/** Both schemes preserve resource references and geometry while selecting their own full-canvas colors. */
#[test]
fn original_source_styles_and_renders_with_script_reference_intact() {
    assert_eq!(ORIGINAL.len(), 577);
    for (appearance, background, foreground) in [
        (Appearance::Light, "#eee", "#000000"),
        (Appearance::Dark, "#222", "#ffffff"),
    ] {
        let context = context(1100.0, 680.0, appearance);
        let document = computed(ORIGINAL, context);
        let display = render_with_context(&document, context).unwrap();
        assert_eq!(document.nodes.len(), 1);
        assert_eq!(document.scripts[0].src.as_deref(), Some("/s.js"));
        assert_eq!(display.canvas_background.as_deref(), Some(background));
        assert_eq!(display.canvas_foreground.as_deref(), Some(foreground));
        assert!(document.formatting.body_box_authored);
        let paragraph = &display.items[0];
        assert_eq!((paragraph.x, paragraph.width), (342.0, 416.0));
        // The body's 25vh top padding separates it from the paragraph's UA 1em top margin, so
        // the margins do not collapse and the paragraph starts 16px below the padding edge.
        assert_eq!(paragraph.y, 170.0 + P_MARGIN);
        assert_eq!(paragraph.line_height, Some(25.6));
        for line in &paragraph.lines {
            assert!((line.x - (342.0 + (416.0 - line.width) / 2.0)).abs() < 0.001);
        }
    }
}

/** Viewport height and content-box width remain independent across reflow contexts. */
#[test]
fn body_edges_auto_margins_and_vh_use_the_current_context() {
    for (width, height, x, content) in [(640.0, 400.0, 112.0, 416.0), (320.0, 800.0, 32.0, 256.0)] {
        let context = context(width, height, Appearance::Light);
        let document = computed(ORIGINAL, context);
        let display = render_with_context(&document, context).unwrap();
        assert_eq!((display.items[0].x, display.items[0].width), (x, content));
        assert_eq!(display.items[0].y, height / 4.0 + P_MARGIN);
        assert_eq!(document.formatting.body.style.padding[2], 32.0);
    }
}

/** Unitless factors inherit into changed fonts while declared percentages and lengths inherit fixed points. */
#[test]
fn line_height_inheritance_preserves_factor_and_computed_length_distinction() {
    for (declaration, line_height, used) in [
        ("1.6", LineHeight::Number(1.6), 51.2),
        ("160%", LineHeight::Length(25.6), 25.6),
        ("24px", LineHeight::Length(24.0), 24.0),
    ] {
        let document = computed(
            &format!(
                "<style>body{{line-height:{declaration}}}p{{font-size:2em}}</style><p>Readable</p>"
            ),
            RenderContext::default(),
        );
        assert_eq!(document.nodes[0].style.font_size, 32.0);
        assert_eq!(document.nodes[0].style.line_height, line_height);
        assert_eq!(
            render_with_context(&document, RenderContext::default())
                .unwrap()
                .items[0]
                .line_height,
            Some(used)
        );
    }
}

/** A shorthand's omitted fields reset, but independently important longhands keep their priority. */
#[test]
fn font_shorthand_expands_before_priority_and_dependency_resolution() {
    for (css, bold, height) in [
        (
            "font-weight:bold;line-height:2;font:20px system-ui,sans-serif",
            false,
            LineHeight::Normal,
        ),
        (
            "line-height:2!important;font:20px system-ui",
            false,
            LineHeight::Number(2.0),
        ),
        (
            "font: bold 20px/1.8 system-ui;line-height:24px",
            true,
            LineHeight::Length(24.0),
        ),
    ] {
        let document = computed(
            &format!("<p style='{css};padding:2em'>Readable</p>"),
            RenderContext::default(),
        );
        let style = &document.nodes[0].style;
        assert_eq!(
            (style.font_size, style.bold, style.line_height),
            (20.0, bold, height)
        );
        assert_eq!(style.padding, [40.0; 4]);
    }
}

/** HTML and BODY are separate subjects in selector ancestry and never appear as native targets. */
#[test]
fn virtual_wrapper_attributes_and_descendants_do_not_create_nodes() {
    let document = computed(
        "<html id=root><style>#root{color:blue}html body.page{font-size:20px}body p{color:red}html p{line-height:2}</style><body class=page><p>Readable</p></body></html>",
        RenderContext::default(),
    );
    assert_eq!(document.nodes.len(), 1);
    assert_eq!(
        document.formatting.html.style.color.as_deref(),
        Some("blue")
    );
    assert_eq!(document.formatting.body.style.font_size, 20.0);
    assert_eq!(document.nodes[0].style.color.as_deref(), Some("red"));
    assert_eq!(document.nodes[0].style.line_height, LineHeight::Number(2.0));
}

/** The body background propagates only when the root is transparent; decoration has no identity. */
#[test]
fn canvas_background_propagation_preserves_distinct_body_paint() {
    let context = RenderContext::default();
    let propagated = computed(
        "<style>body{background:red;padding:10px;border:1px solid blue}</style><p>Readable</p>",
        context,
    );
    let display = render_with_context(&propagated, context).unwrap();
    assert_eq!(display.canvas_background.as_deref(), Some("red"));
    assert_eq!(display.body_decoration.unwrap().color, None);
    let separate = computed(
        "<style>html{background:white}body{background:red;padding:10px}</style><p>Readable</p>",
        context,
    );
    let display = render_with_context(&separate, context).unwrap();
    assert_eq!(display.canvas_background.as_deref(), Some("white"));
    assert_eq!(
        display.body_decoration.unwrap().color.as_deref(),
        Some("red")
    );
}

/** A single author-supported scheme wins even when the host prefers the other one. */
#[test]
fn scheme_negotiation_controls_colors_and_readable_link_defaults() {
    let context = context(640.0, 400.0, Appearance::Dark);
    let document = computed(
        "<style>html{color-scheme:light;background:light-dark(white,black)}</style><p><a href='/'>Readable</a></p>",
        context,
    );
    assert_eq!(
        document.formatting.html.style.color_scheme,
        ColorScheme::Light
    );
    assert_eq!(
        document.formatting.html.style.used_scheme,
        Appearance::Light
    );
    let display = render_with_context(&document, context).unwrap();
    assert_eq!(display.canvas_background.as_deref(), Some("white"));
    assert_eq!(
        display
            .items
            .iter()
            .find(|item| item.role == "link")
            .unwrap()
            .color
            .as_deref(),
        Some("#0000ee")
    );
}

/** Unsupported root, body and value grammar is dropped with a diagnostic; supported grammar applies. */
#[test]
fn unsupported_structural_grammar_is_dropped_and_supported_grammar_applies() {
    let styled = |css: &str| {
        let source = parse("<p>Readable</p>").unwrap();
        compute_styles_with_context(&source, &[css.into()], RenderContext::default()).unwrap()
    };
    for (css, diagnostic) in [
        (
            "html{padding:1px}",
            "Root element box styles are not supported.",
        ),
        (
            // A flex or grid body is laid out (Eye gives it a container node); inline is not.
            "body{display:inline}",
            "Root and body display types other than block are not supported.",
        ),
        (
            "p{padding:auto}",
            "A CSS value for padding is not supported and was ignored.",
        ),
    ] {
        let document = styled(css);
        assert_eq!(document.diagnostics, [diagnostic], "{css}");
        assert_eq!(document.formatting.html.style.padding, [0.0; 4], "{css}");
        assert!(!document.formatting.body.style.display_flex, "{css}");
        assert_eq!(document.nodes[0].style.padding, [0.0; 4], "{css}");
    }
    let font = styled("p{font:italic 16px serif}");
    assert!(font.diagnostics.is_empty());
    assert_eq!(font.nodes[0].style.font_size, 16.0);
    let positioned = styled("p{position:absolute}");
    assert_eq!(positioned.nodes[0].style.position, Position::Absolute);
    // An absolute box's margins never collapse: static position 8px plus its own 16px margin.
    let display = render_with_context(&positioned, RenderContext::default()).unwrap();
    assert_eq!(
        (display.items[0].x, display.items[0].y),
        (BODY_MARGIN, 24.0)
    );
    let scheme = styled("html{color-scheme:only dark}");
    assert_eq!(scheme.formatting.html.style.color_scheme, ColorScheme::Dark);
}

/** Scheme defaults cover the page even when the author supplies no explicit color literals. */
#[test]
fn a_dark_only_root_has_matching_default_canvas_and_foreground() {
    let context = RenderContext::default();
    let document = computed(
        "<style>html{color-scheme:dark}</style><p>Readable</p>",
        context,
    );
    let display = render_with_context(&document, context).unwrap();
    assert_eq!(display.canvas_background.as_deref(), Some("#000000"));
    assert_eq!(display.canvas_foreground.as_deref(), Some("#ffffff"));
    assert_eq!(display.items[0].color.as_deref(), Some("#ffffff"));
}

/** Alignment shifts the same engine runs that supply individual link hit geometry. */
#[test]
fn centered_mixed_inline_content_keeps_link_paint_and_target_geometry_together() {
    let context = RenderContext::default();
    let document = computed(
        "<style>p{text-align:center;width:300px}</style><p>Before <a href='/'>linked words</a> after</p>",
        context,
    );
    let display = render_with_context(&document, context).unwrap();
    let link = display
        .items
        .iter()
        .find(|item| item.role == "link")
        .unwrap();
    let line = &link.lines[0];
    assert_eq!(link.x, line.x);
    assert_eq!(link.width, line.width);
    let first = display.items[0].lines.first().unwrap();
    let right = display
        .items
        .iter()
        .flat_map(|item| &item.lines)
        .map(|line| line.x + line.width)
        .fold(0.0, f64::max);
    assert!((first.x - BODY_MARGIN - (300.0 - (right - first.x)) / 2.0).abs() < 0.001);
}

/** Native dark preference does not place semantic white text onto an authored light canvas in normal scheme. */
#[test]
fn authored_normal_scheme_background_retains_css_light_foreground_in_dark_window() {
    let context = context(640.0, 400.0, Appearance::Dark);
    let document = computed(
        "<style>html{background:#eee}</style><div><p>Readable</p></div>",
        context,
    );
    let display = render_with_context(&document, context).unwrap();
    assert_eq!(display.canvas_background.as_deref(), Some("#eee"));
    assert_eq!(display.canvas_foreground.as_deref(), Some("#000000"));
    assert!(
        display
            .items
            .iter()
            .all(|item| item.color.as_deref() == Some("#000000"))
    );
}

/** Closed enum fields cannot be independently forged into an impossible single-scheme selection. */
#[test]
fn forged_single_scheme_used_appearance_is_refused() {
    for (color_scheme, used_scheme) in [
        (ColorScheme::Light, Appearance::Dark),
        (ColorScheme::Dark, Appearance::Light),
    ] {
        let mut document = parse("<p>Readable</p>").unwrap();
        document.nodes[0].style.color_scheme = color_scheme;
        document.nodes[0].style.used_scheme = used_scheme;
        assert!(render_with_context(&document, RenderContext::default()).is_err());
    }
}

/** Authored link declarations keep their priority over the scheme's UA default even inside a colored canvas. */
#[test]
fn explicit_link_color_wins_over_scheme_default_and_descendants_inherit_it() {
    let context = context(640.0, 400.0, Appearance::Dark);
    let document = computed(
        "<style>html{color-scheme:light dark;background:light-dark(#eee,#222)}a{color:red}</style><p><a href='/'><span>Authored link</span></a></p>",
        context,
    );
    let display = render_with_context(&document, context).unwrap();
    let link = display
        .items
        .iter()
        .find(|item| item.role == "link")
        .unwrap();
    assert_eq!(link.color.as_deref(), Some("red"));
    assert_eq!(
        display
            .items
            .iter()
            .find(|item| item.tag == "span")
            .unwrap()
            .color
            .as_deref(),
        Some("red")
    );
}

/** An authored normal-scheme canvas uses light defaults while an unstyled page keeps native semantic link colors. */
#[test]
fn authored_normal_background_uses_light_link_defaults_in_dark_window() {
    let context = context(640.0, 400.0, Appearance::Dark);
    let document = computed(
        "<style>html{background:#eee}</style><p><a href='/'>Link</a></p>",
        context,
    );
    let display = render_with_context(&document, context).unwrap();
    assert_eq!(
        display
            .items
            .iter()
            .find(|item| item.role == "link")
            .unwrap()
            .color
            .as_deref(),
        Some("#0000ee")
    );
    let unstyled = computed("<p><a href='/'>Link</a></p>", context);
    assert_eq!(
        unstyled
            .nodes
            .iter()
            .find(|node| node.href.is_some())
            .unwrap()
            .style
            .color,
        None
    );
}

/** Nested inline wrappers keep the link's native target on the aligned descendant paint, including with adjacent text. */
#[test]
fn nested_link_targets_follow_center_and_right_aligned_span_paint() {
    for alignment in ["center", "right"] {
        for (before, after) in [("", ""), ("Before ", " after")] {
            let context = RenderContext::default();
            let document = computed(
                &format!(
                    "<style>p{{text-align:{alignment};width:300px}}</style><p>{before}<a href='/'><span>Link</span></a>{after}</p>"
                ),
                context,
            );
            let display = render_with_context(&document, context).unwrap();
            let link = display
                .items
                .iter()
                .find(|item| item.role == "link")
                .unwrap();
            let span = display
                .items
                .iter()
                .find(|item| item.tag == "span")
                .unwrap();
            assert!(link.lines.is_empty());
            assert_eq!((link.x, link.width), (span.lines[0].x, span.lines[0].width));
            assert_eq!(
                (display.items[0].x, display.items[0].width),
                (BODY_MARGIN, 300.0)
            );
            assert!(link.x > display.items[0].x);
        }
    }
}

/** An inline ancestor retains the union of direct and nested paint rather than losing one side after alignment. */
#[test]
fn aligned_link_bounds_union_direct_text_and_nested_inline_descendants() {
    for alignment in ["center", "right"] {
        let context = RenderContext::default();
        let document = computed(
            &format!(
                "<style>p{{text-align:{alignment};width:300px}}</style><p><a href='/'>Before <span>Link</span> after</a></p>"
            ),
            context,
        );
        let display = render_with_context(&document, context).unwrap();
        let link = display
            .items
            .iter()
            .find(|item| item.role == "link")
            .unwrap();
        let left = display
            .items
            .iter()
            .skip(1)
            .flat_map(|item| &item.lines)
            .map(|line| line.x)
            .fold(f64::INFINITY, f64::min);
        let right = display
            .items
            .iter()
            .skip(1)
            .flat_map(|item| &item.lines)
            .map(|line| line.x + line.width)
            .fold(0.0, f64::max);
        assert_eq!((link.x, link.width), (left, right - left));
        assert_eq!(
            (display.items[0].x, display.items[0].width),
            (BODY_MARGIN, 300.0)
        );
    }
}
