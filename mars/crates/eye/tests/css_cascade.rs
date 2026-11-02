/** Best-effort CSS: dropped features, conditions, selectors, variables and agent-safety masks. */
use mars_eye::{
    Appearance, Document, Node, RenderContext, compute_styles, compute_styles_with_context, parse,
};

/** Styles `markup` with one sheet at the default viewport. */
fn styled(markup: &str, sheet: &str) -> Document {
    compute_styles(&parse(markup).unwrap(), &[sheet.into()]).unwrap()
}
/** The node with `id`. */
fn by_id<'a>(document: &'a Document, id: &str) -> &'a Node {
    document
        .nodes
        .iter()
        .find(|node| node.html_id == id)
        .unwrap()
}
/** The color of the node with `id`. */
fn color(document: &Document, id: &str) -> Option<String> {
    by_id(document, id).style.color.clone()
}

#[test]
fn unknown_property_is_dropped_and_the_rest_of_the_sheet_applies() {
    let document = styled(
        "<p id=a>a</p><div id=b>b</div>",
        "p{offset-path:none;color:red} div{opacity:.5}",
    );
    assert_eq!(color(&document, "a").as_deref(), Some("red"));
    assert_eq!(by_id(&document, "b").style.opacity, 0.5);
    assert!(
        document
            .diagnostics
            .iter()
            .any(|d| d.contains("offset-path"))
    );
}

#[test]
fn diagnostics_are_deduplicated_and_bounded() {
    let sheet = (0..100)
        .map(|i| format!("p{{unknown-{i}:1;unknown-0:1}}"))
        .collect::<String>();
    let document = styled("<p>x</p>", &sheet);
    assert_eq!(document.diagnostics.len(), mars_eye::constants::DIAGNOSTICS);
    let mut unique = document.diagnostics.clone();
    unique.dedup();
    assert_eq!(unique.len(), document.diagnostics.len());
}

#[test]
fn unsupported_value_falls_back_to_the_previous_declaration() {
    let document = styled("<p id=a>a</p>", "p{color:red} p{color:frobnicate}");
    assert_eq!(color(&document, "a").as_deref(), Some("red"));
}

#[test]
fn media_queries_follow_the_viewport_and_appearance() {
    let sheet = "@media (min-width: 600px) { #a{color:red} } \
                 @media screen and (max-width: 37.5em) { #a{color:blue} } \
                 @media print { #b{color:red} } \
                 @media (width >= 1000px) and (prefers-color-scheme: dark) { #b{color:navy} }";
    let document = parse("<p id=a>a</p><p id=b>b</p>").unwrap();
    let dark = RenderContext {
        appearance: Appearance::Dark,
        ..RenderContext::default()
    };
    let light = compute_styles(&document, &[sheet.into()]).unwrap();
    assert_eq!(color(&light, "a").as_deref(), Some("red"));
    assert_eq!(color(&light, "b"), None);
    let dark = compute_styles_with_context(&document, &[sheet.into()], dark).unwrap();
    assert_eq!(color(&dark, "b").as_deref(), Some("navy"));
}

#[test]
fn unknown_media_features_are_false() {
    let document = styled(
        "<p id=a>a</p>",
        "@media (min-resolution: 2dppx){p{color:red}}",
    );
    assert_eq!(color(&document, "a"), None);
}

#[test]
fn supports_evaluates_what_eye_parses() {
    let document = styled(
        "<p id=a>a</p><p id=b>b</p>",
        "@supports (display:flex) { #a{color:red} } @supports (display: frob) { #b{color:red} }",
    );
    assert_eq!(color(&document, "a").as_deref(), Some("red"));
    assert_eq!(color(&document, "b"), None);
}

#[test]
fn unknown_at_rules_and_imports_are_dropped() {
    let document = styled(
        "<p id=a>a</p>",
        "@import url(x.css); @font-face{font-family:x;src:url(x)} @keyframes k{from{opacity:0}} \
         @container (min-width: 1px) { p{color:blue} } p{opacity:.5}",
    );
    let node = by_id(&document, "a");
    assert_eq!(node.style.opacity, 0.5);
    assert_eq!(node.style.color, None);
    assert!(document.diagnostics.iter().any(|d| d.contains("@import")));
}

#[test]
fn child_combinator_matches_only_direct_children() {
    let document = styled(
        "<div><p id=a>a</p><section><p id=b>b</p></section></div>",
        "div > p{color:red}",
    );
    assert_eq!(color(&document, "a").as_deref(), Some("red"));
    assert_eq!(color(&document, "b"), None);
}

#[test]
fn general_sibling_matches_every_later_sibling() {
    let document = styled(
        "<div><p id=a>a</p><h2 id=h>h</h2><p id=b>b</p><span id=c>c</span></div>",
        "h2 ~ p{color:red}",
    );
    assert_eq!(color(&document, "a"), None);
    assert_eq!(color(&document, "b").as_deref(), Some("red"));
    assert_eq!(color(&document, "c"), None);
}

#[test]
fn attribute_selectors_compare_values() {
    let document = styled(
        "<a id=a href='https://x.org/a.pdf' class='btn big' lang=en-US>a</a><a id=b>b</a>",
        "[href]{color:red} [lang|=en]{opacity:.5} a[href^='https']{font-size:20px} \
         a[href$='.pdf']{letter-spacing:1px} a[href*='x.org']{z-index:3} a[class~=big]{cursor:pointer} \
         a[class=\"btn big\"]{text-transform:uppercase} [HREF='HTTPS://X.ORG/A.PDF' i]{white-space:nowrap}",
    );
    let a = &by_id(&document, "a").style;
    assert_eq!(a.color.as_deref(), Some("red"));
    assert_eq!(a.opacity, 0.5);
    assert_eq!(a.font_size, 20.0);
    assert_eq!(a.letter_spacing, 1.0);
    assert_eq!(a.z_index, Some(3));
    assert!(a.cursor_pointer);
    assert_eq!(format!("{:?}", a.text_transform), "Uppercase");
    assert_eq!(format!("{:?}", a.white_space), "Nowrap");
    assert_eq!(color(&document, "b"), None);
}

#[test]
fn structural_pseudo_classes_count_element_siblings() {
    let document = styled(
        "<ul><li id=a>a</li><li id=b>b</li><li id=c>c</li><li id=d>d</li></ul>",
        "li:first-child{color:red} li:last-child{color:blue} li:nth-child(2n+3){opacity:.5} \
         li:nth-child(even){font-size:20px}",
    );
    assert_eq!(color(&document, "a").as_deref(), Some("red"));
    assert_eq!(color(&document, "d").as_deref(), Some("blue"));
    assert_eq!(by_id(&document, "c").style.opacity, 0.5);
    assert_eq!(by_id(&document, "b").style.font_size, 20.0);
    assert_eq!(by_id(&document, "d").style.font_size, 20.0);
    assert_eq!(by_id(&document, "a").style.opacity, 1.0);
}

#[test]
fn not_root_and_link_match() {
    let document = styled(
        "<a id=a href=/x>a</a><a id=b>b</a><p id=c class=keep>c</p>",
        ":root{--c:navy} a:link{color:red} p:not(.keep){color:blue} :root p{opacity:.5}",
    );
    assert_eq!(color(&document, "a").as_deref(), Some("red"));
    assert_eq!(color(&document, "b"), None);
    assert_eq!(color(&document, "c"), None);
    assert_eq!(by_id(&document, "c").style.opacity, 0.5);
}

#[test]
fn user_action_pseudo_classes_never_match_but_keep_the_rule() {
    let document = styled(
        "<a id=a href=/x>a</a>",
        "a:hover{color:red} a:focus-visible, a{opacity:.5}",
    );
    assert_ne!(color(&document, "a").as_deref(), Some("red"));
    assert_eq!(by_id(&document, "a").style.opacity, 0.5);
}

#[test]
fn escaped_class_names_match() {
    let document = styled(
        "<p id=a class='md:flex w-1/2'>a</p>",
        ".md\\:flex.w-1\\/2{color:red}",
    );
    assert_eq!(color(&document, "a").as_deref(), Some("red"));
}

#[test]
fn before_rules_style_only_the_generated_box() {
    let document = styled(
        "<p id=a>a</p>",
        "p::before{color:red;content:'x'} p{opacity:.5}",
    );
    assert_eq!(color(&document, "a"), None);
    assert_eq!(by_id(&document, "a").style.opacity, 0.5);
    let generated = &by_id(&document, "a").style.generated;
    assert_eq!(generated.len(), 1);
    assert_eq!(generated[0].content, "x");
    assert_eq!(generated[0].style.color.as_deref(), Some("red"));
}

#[test]
fn other_pseudo_element_rules_never_style_the_element() {
    let document = styled("<p id=a>a</p>", "p::selection{color:red} p{opacity:.5}");
    assert_eq!(color(&document, "a"), None);
    assert_eq!(by_id(&document, "a").style.opacity, 0.5);
    assert!(
        document
            .diagnostics
            .iter()
            .all(|d| !d.contains("pseudo-element"))
    );
}

#[test]
fn dropped_hiding_selector_makes_its_possible_targets_not_actionable() {
    let document = styled(
        "<div id=m class=menu><button id=b type=button>Go</button></div><button id=c type=button>Ok</button>",
        ".menu:state(open){display:none}",
    );
    assert!(!by_id(&document, "m").style.interactive);
    assert!(!by_id(&document, "b").style.interactive);
    assert!(by_id(&document, "c").style.interactive);
    assert!(!by_id(&document, "m").style.hidden);
}

#[test]
fn dropped_hiding_declaration_makes_its_targets_not_actionable() {
    let document = styled(
        "<input id=a><input id=b class=v>",
        ".v{clip-path:inset(50%);color:red}",
    );
    assert!(by_id(&document, "a").style.interactive);
    let b = &by_id(&document, "b").style;
    assert!(!b.interactive);
    assert_eq!(b.color.as_deref(), Some("red"));
}

#[test]
fn hiding_rules_under_unknown_conditions_mask_their_targets() {
    let document = styled(
        "<button id=a type=button>a</button>",
        "@container (min-width: 1px) { #a{display:none} }",
    );
    let a = &by_id(&document, "a").style;
    assert!(!a.hidden);
    assert!(!a.interactive);
}

#[test]
fn offscreen_positioned_controls_are_not_actionable() {
    let document = styled(
        "<input id=a name=trap><input id=b>",
        "#a{position:absolute;left:-99999px;top:0}",
    );
    let a = &by_id(&document, "a").style;
    assert!(!a.interactive);
    assert_eq!(a.inset[3], Some(-mars_eye::constants::CSS_MAX_SCALAR));
    assert!(by_id(&document, "b").style.interactive);
}

#[test]
fn positioning_and_overflow_parse() {
    let document = styled(
        "<div id=a>a</div>",
        "#a{position:sticky;inset:1px 2em auto 10%;z-index:-2;overflow-x:auto;overflow-y:hidden}",
    );
    let a = &by_id(&document, "a").style;
    assert_eq!(format!("{:?}", a.position), "Sticky");
    assert_eq!(a.inset, [Some(1.0), Some(32.0), None, None]);
    assert_eq!(a.z_index, Some(-2));
    assert_eq!(format!("{:?}", a.overflow), "Hidden");
}

#[test]
fn radius_shadow_and_transform_parse() {
    let document = styled(
        "<div id=a>a</div>",
        "#a{width:100px;border-radius:4px 50%;box-shadow:0 1px 2px rgba(0,0,0,.5), inset 1px 1px red;\
         transform:translate(10px, 20px) scale(2)}",
    );
    let a = &by_id(&document, "a").style;
    assert_eq!(a.radius, [4.0, 50.0, 4.0, 50.0]);
    assert_eq!(a.shadows.len(), 2);
    assert_eq!((a.shadows[0].y, a.shadows[0].blur), (1.0, 2.0));
    assert_eq!(a.shadows[0].color, "#00000080");
    assert!(a.shadows[1].inset);
    assert_eq!(a.transform, Some([2.0, 0.0, 0.0, 2.0, 10.0, 20.0]));
}

#[test]
fn background_shorthand_splits_color_and_layers() {
    let document = styled(
        "<div id=a>a</div><div id=b>b</div>",
        "#a{background:url(\"/Img.PNG\") no-repeat, linear-gradient(to right, red, #00f 50%) #fafafa}\
         #b{background-image:radial-gradient(circle at center, white 0%, black 100%)}",
    );
    let a = &by_id(&document, "a").style;
    assert_eq!(a.background.as_deref(), Some("#fafafa"));
    let layers = serde_json::to_value(&a.backgrounds).unwrap();
    assert_eq!(layers[0]["image"]["url"], "/Img.PNG");
    assert_eq!(layers[1]["linear"]["angle"], 90.0);
    assert_eq!(layers[1]["linear"]["stops"][1]["color"], "#00f");
    assert_eq!(layers[1]["linear"]["stops"][1]["position"], 0.5);
    let b = serde_json::to_value(&by_id(&document, "b").style.backgrounds).unwrap();
    assert_eq!(b[0]["radial"]["stops"].as_array().unwrap().len(), 2);
}

#[test]
fn text_properties_parse() {
    let document = styled(
        "<a id=a href=/x>a</a><p id=p>p</p>",
        "a{text-decoration:none;font-weight:300} p{font-weight:650;letter-spacing:-.5px;white-space:pre-wrap;\
         text-transform:capitalize;font-family:Menlo, monospace}",
    );
    let a = &by_id(&document, "a").style;
    assert!(a.text_decoration_none);
    assert!(a.cursor_pointer);
    assert_eq!((a.font_weight, a.bold), (300, false));
    let p = &by_id(&document, "p").style;
    assert_eq!((p.font_weight, p.bold), (650, true));
    assert_eq!(p.letter_spacing, -0.5);
    assert_eq!(format!("{:?}", p.white_space), "PreWrap");
    assert_eq!(format!("{:?}", p.text_transform), "Capitalize");
    assert_eq!(format!("{:?}", p.font_family), "Monospace");
}

#[test]
fn per_side_borders_resolve_with_their_style() {
    let document = styled(
        "<div id=a>a</div><div id=b>b</div><input id=c><div id=d>d</div>",
        "#a{border-bottom:2px solid red} #b{border:1px dashed #333} #c{border:none} #d{border-width:3px}",
    );
    let a = &by_id(&document, "a").style;
    assert_eq!(a.border_widths, [0.0, 0.0, 2.0, 0.0]);
    assert_eq!(a.border, 0.0);
    assert_eq!(a.border_colors[2].as_deref(), Some("red"));
    let b = &by_id(&document, "b").style;
    assert_eq!((b.border, b.border_color.as_deref()), (1.0, Some("#333")));
    assert!(by_id(&document, "c").style.border_style_none);
    assert_eq!(by_id(&document, "d").style.border, 0.0);
}

#[test]
fn colors_accept_css_syntax() {
    let cases = [
        ("rgb(255 0 0 / 50%)", "#ff000080"),
        ("rgba(0, 128, 0, 1)", "#008000"),
        ("hsl(120deg 100% 25%)", "#008000"),
        ("rebeccapurple", "#663399"),
        ("#0f08", "#00ff0088"),
        ("#ABCDEF", "#abcdef"),
        ("oklch(62.8% 0.2577 29.23)", "#ff0000"),
    ];
    for (source, expected) in cases {
        let document = styled("<p id=a>a</p>", &format!("p{{color:{source}}}"));
        assert_eq!(color(&document, "a").as_deref(), Some(expected), "{source}");
    }
}

#[test]
fn current_color_and_transparent_resolve() {
    let document = styled(
        "<p id=a>a</p>",
        "p{color:teal;border:1px solid currentColor;background-color:transparent}",
    );
    let a = &by_id(&document, "a").style;
    assert_eq!(a.border_color.as_deref(), Some("teal"));
    assert_eq!(a.background.as_deref(), Some("transparent"));
}

#[test]
fn custom_properties_substitute_and_inherit() {
    let document = styled(
        "<div id=d><p id=a>a</p><p id=b>b</p><p id=c>c</p></div>",
        ":root{--Brand: red; --pad: 4px 8px} #d{--ok: var(--Brand)} #a{color:var(--ok)} \
         #b{color:var(--missing, blue);padding:var(--pad)} #c{--x:var(--y);--y:var(--x);color:var(--x)}",
    );
    assert_eq!(color(&document, "a").as_deref(), Some("red"));
    assert_eq!(color(&document, "b").as_deref(), Some("blue"));
    assert_eq!(by_id(&document, "b").style.padding, [4.0, 8.0, 4.0, 8.0]);
    assert_eq!(color(&document, "c"), None);
}

#[test]
fn important_and_wide_keywords_resolve() {
    let document = styled(
        "<div id=d><p id=a style='color:blue'>a</p><h1 id=h>h</h1><span id=c>c</span></div>",
        "#d{color:green;font-size:20px} p{color:red !important} h1{font-size:inherit} #c{color:initial}",
    );
    assert_eq!(color(&document, "a").as_deref(), Some("red"));
    assert_eq!(by_id(&document, "h").style.font_size, 20.0);
    assert_eq!(color(&document, "c"), None);
}

#[test]
fn units_and_calc_resolve() {
    let document = styled(
        "<div id=a>a</div>",
        "#a{width:calc(10px + 2em);padding-left:12pt;padding-right:10vw;max-width:min(500px, 50vw);margin-top:clamp(1px, 2rem, 20px)}",
    );
    let a = &by_id(&document, "a").style;
    assert_eq!(a.width, Some(42.0));
    assert_eq!(a.padding[3], 16.0);
    assert_eq!(a.padding[1], 110.0);
    assert_eq!(a.max_width, Some(500.0));
    assert_eq!(a.margin[0], 20.0);
}

#[test]
fn css_nesting_resolves_parents() {
    let document = styled(
        "<div class=a><p id=x class=b>x</p></div><p id=y class=b>y</p>",
        ".a { color: green; .b { color: red } & > .b { opacity: .5 } @media screen { .b { font-size: 20px } } }",
    );
    let x = &by_id(&document, "x").style;
    assert_eq!(x.color.as_deref(), Some("red"));
    assert_eq!(x.opacity, 0.5);
    assert_eq!(x.font_size, 20.0);
    assert_eq!(color(&document, "y"), None);
}

#[test]
fn chromium_user_agent_defaults_apply() {
    let document = compute_styles(
        &parse("<h1 id=h>h</h1><p id=p>p</p><ul id=u><li>x</li></ul><dl><dd id=d>d</dd></dl><span id=s>s</span><button id=b>b</button>")
            .unwrap(),
        &[],
    )
    .unwrap();
    let h = &by_id(&document, "h").style;
    assert_eq!(h.font_size, 32.0);
    assert!((h.margin[0] - 0.67 * 32.0).abs() < 1e-9);
    assert!(h.bold);
    assert_eq!(by_id(&document, "p").style.margin, [16.0, 0.0, 16.0, 0.0]);
    assert_eq!(by_id(&document, "u").style.padding[3], 40.0);
    assert_eq!(by_id(&document, "d").style.margin[3], 40.0);
    assert!(by_id(&document, "s").style.inline);
    assert!(by_id(&document, "b").style.inline);
    assert_eq!(document.formatting.body.style.margin, [8.0; 4]);
    assert_eq!(format!("{:?}", h.font_family), "Serif");
}

#[test]
fn author_styles_override_user_agent_defaults() {
    let document = styled("<p id=p>p</p>", "*{margin:0}");
    assert_eq!(by_id(&document, "p").style.margin, [0.0; 4]);
}

#[test]
fn oversized_stylesheet_is_skipped_not_refused() {
    let document = parse("<p id=a>a</p>").unwrap();
    let sheet = "p{color:red}".repeat(mars_eye::constants::STYLE_BYTES);
    let document = compute_styles(&document, &[sheet, "p{opacity:.5}".into()]).unwrap();
    assert_eq!(color(&document, "a"), None);
    assert_eq!(by_id(&document, "a").style.opacity, 0.5);
    assert!(document.diagnostics.iter().any(|d| d.contains("skipped")));
}

#[test]
fn rules_past_the_bound_are_dropped_not_refused() {
    let document = parse("<p id=a>a</p>").unwrap();
    let rules = ".x{color:blue}".repeat(mars_eye::constants::CSS_RULES);
    let document = compute_styles(&document, &[rules, "p{color:red}".into()]).unwrap();
    assert_eq!(color(&document, "a"), None);
    assert!(document.diagnostics.iter().any(|d| d.contains("CSS rules")));
}

#[test]
fn hostile_sheets_never_panic() {
    let document = parse("<div><p class=a>x</p></div>").unwrap();
    let sheets = [
        "p{color:red",
        "}}}{{{p{",
        "/* unterminated",
        "p{content:\"}\";color:red}",
        "@media (",
        "@media screen{@media screen{@media screen{@media screen{@media screen{@media screen{@media screen{@media screen{@media screen{@media screen{p{color:red}}}}}}}}}}}",
        "a[href=\"x]{}",
        ":nth-child(99999999999999999999n){}",
        "p{width:calc(((((((((((1px)))))))))))}",
        "p{color:rgb(1e308,1e308,1e308)}",
        "p{--a:var(--a) var(--a) var(--a);color:var(--a)}",
        "é{color:red} .é\\0 {color:red} p\\{color:red}",
        "p{transform:rotate(90deg) skewX(90deg)}",
        "p{border-radius:-5px;box-shadow:1px}",
        ":not(:not(:not(p))){color:red}",
    ];
    for sheet in sheets {
        let _ = compute_styles(&document, &[sheet.into()]);
    }
}

#[test]
fn hostile_selectors_stay_fast() {
    let markup = "<div>".repeat(30) + &"<p>x</p>".repeat(200) + &"</div>".repeat(30);
    let document = parse(&markup).unwrap();
    let sheet = format!("{} p{{color:red}}", "div ".repeat(60)).repeat(16)
        + &format!("{}p{{color:blue}}", "p ~ ".repeat(60)).repeat(16);
    let started = std::time::Instant::now();
    compute_styles(&document, &[sheet]).unwrap();
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
}

#[test]
fn unknown_and_custom_elements_are_inline() {
    let document = compute_styles(
        &parse("<shreddit-post id=a>a</shreddit-post><frob id=b>b</frob><div id=c>c</div>")
            .unwrap(),
        &[],
    )
    .unwrap();
    assert!(by_id(&document, "a").style.inline);
    assert!(by_id(&document, "b").style.inline);
    assert!(!by_id(&document, "c").style.inline);
}

#[test]
fn bucketed_rules_still_check_every_simple_selector() {
    let document = styled(
        "<p id=a class='x y'>a</p><p id=b class=y>b</p><p id=c>c</p>",
        "#a.x{color:red} #b.x{color:blue} .y.x{opacity:.5} p:not(#a){font-size:20px}",
    );
    assert_eq!(color(&document, "a").as_deref(), Some("red"));
    assert_eq!(color(&document, "b"), None);
    assert_eq!(by_id(&document, "a").style.opacity, 0.5);
    assert_eq!(by_id(&document, "b").style.opacity, 1.0);
    assert_eq!(by_id(&document, "c").style.font_size, 20.0);
}
