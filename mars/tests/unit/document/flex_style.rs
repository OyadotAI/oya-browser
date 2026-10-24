/** Flex styles are checked through public cascade and private-value validation boundaries. */
use mars::document::{
    AlignItems, BoxSizing, ComputedStyle, FlexBasis, FlexDirection, FlexWrap, JustifyContent,
    compute_styles, parse, validate_styles,
};
use serde_json::json;
/** A single stable author identity exposes computed values independently of layout algorithms. */
fn style(declarations: &str) -> ComputedStyle {
    let document = parse("<div id=x></div>").unwrap();
    compute_styles(&document, &[format!("#x{{{declarations}}}")])
        .unwrap()
        .nodes[0]
        .style
        .clone()
}
/** Defaults distinguish automatic main/minimum sizes from explicit zero and do not create flex containers. */
#[test]
fn flex_defaults_are_explicit_and_preserve_legacy_styles() {
    let computed = style("");
    assert_eq!(computed, ComputedStyle::default());
    assert_eq!(computed.box_sizing, BoxSizing::ContentBox);
    assert!(!computed.display_flex);
    assert_eq!(computed.flex_direction, FlexDirection::Row);
    assert_eq!(computed.flex_wrap, FlexWrap::NoWrap);
    assert_eq!(computed.flex_basis, FlexBasis::Auto);
    assert_eq!((computed.flex_grow, computed.flex_shrink), (0.0, 1.0));
    assert_eq!((computed.row_gap, computed.column_gap), (0.0, 0.0));
    assert_eq!(computed.justify_content, JustifyContent::Start);
    assert_eq!(computed.align_items, AlignItems::Stretch);
    assert_eq!(computed.min_width, None);
}
/** Flex shorthand defaults follow the pinned source, retaining zero percent versus explicit zero pixels. */
#[test]
fn flex_shorthand_supports_standard_bounded_forms_and_zero_distinction() {
    for (value, grow, shrink, basis) in [
        ("initial", 0.0, 1.0, FlexBasis::Auto),
        ("none", 0.0, 0.0, FlexBasis::Auto),
        ("auto", 1.0, 1.0, FlexBasis::Auto),
        ("1", 1.0, 1.0, FlexBasis::Percent(0.0)),
        ("0", 0.0, 1.0, FlexBasis::Percent(0.0)),
        ("2 3", 2.0, 3.0, FlexBasis::Percent(0.0)),
        ("280px", 1.0, 1.0, FlexBasis::Pixels(280.0)),
        ("25%", 1.0, 1.0, FlexBasis::Percent(25.0)),
        ("2 40px", 2.0, 1.0, FlexBasis::Pixels(40.0)),
        ("2 3 0", 2.0, 3.0, FlexBasis::Pixels(0.0)),
        ("2 3 0%", 2.0, 3.0, FlexBasis::Percent(0.0)),
        ("2 3 auto", 2.0, 3.0, FlexBasis::Auto),
        ("40px 2 3", 2.0, 3.0, FlexBasis::Pixels(40.0)),
    ] {
        let computed = style(&format!("flex:{value}"));
        assert_eq!(
            (
                computed.flex_grow,
                computed.flex_shrink,
                computed.flex_basis
            ),
            (grow, shrink, basis),
            "{value}"
        );
    }
}
/** A shorthand resets all its longhands at its declaration position, in either source order. */
#[test]
fn flex_shorthand_resets_before_cascade_selection() {
    let computed = style("flex-shrink:9;flex-basis:20px;flex-grow:8;flex:2");
    assert_eq!(
        (
            computed.flex_grow,
            computed.flex_shrink,
            computed.flex_basis
        ),
        (2.0, 1.0, FlexBasis::Percent(0.0))
    );
    let computed = style("flex:2;flex-grow:8;flex-shrink:9;flex-basis:20px");
    assert_eq!(
        (
            computed.flex_grow,
            computed.flex_shrink,
            computed.flex_basis
        ),
        (8.0, 9.0, FlexBasis::Pixels(20.0))
    );
    let computed = style("flex-shrink:9!important;flex:2");
    assert_eq!(
        (
            computed.flex_grow,
            computed.flex_shrink,
            computed.flex_basis
        ),
        (2.0, 9.0, FlexBasis::Percent(0.0))
    );
}
/** Source order remains meaningful across rules and inline declarations, while importance and specificity still win. */
#[test]
fn expanded_declarations_keep_importance_specificity_and_inline_order() {
    let document = parse("<div id=x class=c style='flex:3;flex-basis:7px'></div>").unwrap();
    let computed = compute_styles(
        &document,
        &["#x{flex:2 3 40px}.c{flex:4 5 60px!important}#x{flex-shrink:9!important}".into()],
    )
    .unwrap()
    .nodes[0]
        .style
        .clone();
    assert_eq!(
        (
            computed.flex_grow,
            computed.flex_shrink,
            computed.flex_basis
        ),
        (4.0, 9.0, FlexBasis::Pixels(60.0))
    );
    let document = parse("<div id=x style='flex-basis:7px;flex:3'></div>").unwrap();
    let computed = compute_styles(&document, &[]).unwrap().nodes[0]
        .style
        .clone();
    assert_eq!(computed.flex_basis, FlexBasis::Percent(0.0));
    let document = parse("<div id=x></div>").unwrap();
    let computed = compute_styles(&document, &["#x{flex-grow:9}#x{flex:1}".into()])
        .unwrap()
        .nodes[0]
        .style
        .clone();
    assert_eq!(computed.flex_grow, 1.0);
}
/** Margin and padding shorthands no longer depend on alphabetical property application. */
#[test]
fn box_shorthand_order_preserves_winning_font_relative_lengths() {
    let computed =
        style("margin-left:5px;margin:1em 2em;padding-top:1px;padding:3px 4px;font-size:20px");
    assert_eq!(computed.margin, [20.0, 40.0, 20.0, 40.0]);
    assert_eq!(computed.padding, [3.0, 4.0, 3.0, 4.0]);
    let computed =
        style("margin:1em 2em;margin-left:5px;padding:3px 4px;padding-top:1px;font-size:20px");
    assert_eq!(computed.margin, [20.0, 40.0, 20.0, 5.0]);
    assert_eq!(computed.padding, [1.0, 4.0, 3.0, 4.0]);
    assert_eq!(
        style("margin-left:5px!important;margin:1px").margin,
        [1.0, 1.0, 1.0, 5.0]
    );
}
/** Gap expansion retains row/column order and longhand overrides in either declaration direction. */
#[test]
fn gap_shorthand_and_longhands_keep_source_order() {
    let computed = style("row-gap:2px;gap:3px 4px");
    assert_eq!((computed.row_gap, computed.column_gap), (3.0, 4.0));
    let computed = style("gap:3px 4px;row-gap:2px");
    assert_eq!((computed.row_gap, computed.column_gap), (2.0, 4.0));
    let computed = style("column-gap:8px!important;gap:5px");
    assert_eq!((computed.row_gap, computed.column_gap), (5.0, 8.0));
    let computed = style("gap:0");
    assert_eq!((computed.row_gap, computed.column_gap), (0.0, 0.0));
}
/** Supported structural longhands are independent of inherited text alignment and native interaction guards. */
#[test]
fn flex_longhands_and_box_sizing_compute_exact_values() {
    let computed = style(
        "display:flex;box-sizing:border-box;flex-direction:row;flex-wrap:wrap;flex-basis:50%;flex-grow:2.5;flex-shrink:0.5;justify-content:space-between;align-items:center;min-width:0;width:100%;max-width:620px",
    );
    assert!(computed.display_flex && !computed.inline);
    assert_eq!(computed.box_sizing, BoxSizing::BorderBox);
    assert_eq!(computed.flex_wrap, FlexWrap::Wrap);
    assert_eq!(computed.flex_basis, FlexBasis::Percent(50.0));
    assert_eq!((computed.flex_grow, computed.flex_shrink), (2.5, 0.5));
    assert_eq!(computed.justify_content, JustifyContent::SpaceBetween);
    assert_eq!(computed.align_items, AlignItems::Center);
    assert_eq!(computed.min_width, Some(0.0));
    assert_eq!(
        (computed.width, computed.percent_width, computed.max_width),
        (Some(100.0), true, Some(620.0))
    );
    assert_eq!(style("display:flex;flex-direction:column;justify-content:flex-end;align-items:flex-start;min-width:auto").flex_direction,FlexDirection::Column);
}
/** Hiding and input suppression survive valid flex declarations on descendants. */
#[test]
fn flex_cannot_override_hidden_and_inert_ancestor_guards() {
    let document=parse("<div id=parent hidden><div id=child><input id=secret type=hidden><button id=go>Go</button></div></div>").unwrap();
    let computed=compute_styles(&document,&["#parent{display:flex;pointer-events:none}#child{display:flex;visibility:visible;pointer-events:auto}".into()]).unwrap();
    assert!(computed.nodes.iter().all(|node| node.hidden()));
    assert!(computed.nodes.iter().all(|node| !node.style.interactive));
}
/// Unsupported structures and malformed scalar forms are dropped with a diagnostic naming the
/// property, and never invent geometry: the result equals the style without that declaration.
#[test]
fn unsupported_flex_values_and_excluded_structures_are_dropped_with_a_diagnostic() {
    let computed = |css: &str| {
        let document = parse("<div id=x></div>").unwrap();
        compute_styles(&document, &[format!("#x{{{css}}}")]).unwrap()
    };
    for (declaration, kept, named) in [
        (
            "display:flex;flex-direction:column;flex-wrap:wrap",
            "display:flex;flex-direction:column",
            "Wrapped flex columns",
        ),
        ("flex:1 auto 2", "", "flex"),
        ("flex:1 2 3", "", "flex"),
        ("flex:1 2 0 0", "", "flex"),
        ("flex-basis:1em", "", "flex-basis"),
        ("flex-basis:content", "", "flex-basis"),
        ("flex-grow:-1", "", "flex-grow"),
        ("flex-shrink:NaN", "", "flex-shrink"),
        ("flex-grow:4097", "", "flex-grow"),
        ("gap:10", "", "gap"),
        ("gap:1px 2px 3px", "", "gap"),
        ("align-content:stretch", "", "align-content"),
        ("padding:auto", "", "padding"),
    ] {
        let document = computed(declaration);
        assert_eq!(document.diagnostics.len(), 1, "{declaration}");
        assert!(document.diagnostics[0].contains(named), "{declaration}");
        assert_eq!(
            document.nodes[0].style,
            computed(kept).nodes[0].style,
            "{declaration}"
        );
    }
}
/** Deserializing omitted fields retains defaults while unknown structural variants cannot enter a computed style. */
#[test]
fn computed_style_serde_preserves_defaults_and_rejects_unknown_variants() {
    let computed: ComputedStyle = serde_json::from_value(json!({})).unwrap();
    assert_eq!(computed, ComputedStyle::default());
    for value in [
        json!({"box_sizing":"padding_box"}),
        json!({"flex_wrap":"wrap_reverse"}),
        json!({"flex_direction":"sideways"}),
        json!({"flex_basis":{"content":0}}),
        json!({"height":100}),
    ] {
        assert!(serde_json::from_value::<ComputedStyle>(value).is_err());
    }
}
/** Rust/private-wire callers are checked before their computed values reach native rendering. */
#[test]
fn private_computed_styles_reject_nonfinite_negative_and_excessive_values() {
    let cases: &[fn(&mut ComputedStyle)] = &[
        |s| s.font_size = f64::NAN,
        // Zero is `font-size: 0`, which hides text; sizes between it and the minimum are invalid.
        |s| s.font_size = 4.0,
        |s| s.font_size = 73.0,
        |s| s.border = 9.0,
        |s| s.margin[0] = -4097.0,
        |s| s.padding[0] = f64::INFINITY,
        |s| s.width = Some(4097.0),
        |s| s.max_width = Some(-1.0),
        |s| s.min_width = Some(4097.0),
        |s| s.flex_grow = -1.0,
        |s| s.flex_shrink = f64::NAN,
        |s| s.row_gap = f64::INFINITY,
        |s| s.column_gap = -1.0,
        |s| s.flex_basis = FlexBasis::Percent(f64::NAN),
        |s| s.flex_basis = FlexBasis::Pixels(-1.0),
        |s| s.percent_width = true,
        |s| s.align = "unknown".into(),
        |s| s.color = Some("url(private)".into()),
        |s| {
            s.display_flex = true;
            s.inline = true;
        },
    ];
    for change in cases {
        let mut document = parse("<div id=x></div>").unwrap();
        change(&mut document.nodes[0].style);
        assert_eq!(
            validate_styles(&document).unwrap_err().code,
            "invalid_request"
        );
        assert!(compute_styles(&document, &[]).is_err());
    }
}
