/** `@container` style queries, height and shape size queries, and the `cq*` units. */
use mars_eye::{ComputedStyle, Document, compute_styles, parse};

/** Styles `markup` with one sheet through the public cascade. */
fn styled(markup: &str, sheet: &str) -> Document {
    compute_styles(&parse(markup).unwrap(), &[sheet.to_owned()]).unwrap()
}

/** The computed style of the element with `id`. */
fn style<'a>(document: &'a Document, id: &str) -> &'a ComputedStyle {
    &document
        .nodes
        .iter()
        .find(|node| node.html_id == id)
        .unwrap()
        .style
}

/** ChatGPT's theme switch: a custom property on an ancestor picks the rules that apply. */
const THEME: &str = "@container style(--theme-variant:light){p{order:1}}\
                     @container style(--theme-variant:dark){p{order:2}}";

#[test]
fn an_unnamed_style_query_reads_the_parents_custom_property() {
    let light = styled(
        "<div style='--theme-variant: light'><p id=p>x</p></div>",
        THEME,
    );
    assert_eq!(style(&light, "p").order, 1);
    let dark = styled(
        "<div style='--theme-variant:dark'><p id=p>x</p></div>",
        THEME,
    );
    assert_eq!(style(&dark, "p").order, 2);
}

#[test]
fn a_false_style_query_applies_nothing_and_masks_nothing() {
    let document = styled(
        "<div style='--theme-variant:light'><a id=a href=/x>a</a></div>",
        "@container style(--theme-variant:dark){a{display:none}}",
    );
    let a = style(&document, "a");
    assert!(!a.hidden && a.interactive);
}

#[test]
fn a_style_query_without_a_value_holds_when_the_property_is_set() {
    let sheet = "@container style(--gallery-navbar){p{order:3}}";
    let set = styled("<div style='--gallery-navbar:1'><p id=p>x</p></div>", sheet);
    assert_eq!(style(&set, "p").order, 3);
    let unset = styled("<div><p id=p>x</p></div>", sheet);
    assert_eq!(style(&unset, "p").order, 0);
}

#[test]
fn style_query_names_and_values_keep_their_case() {
    let sheet = "@container STYLE(--heroLayout: Row){p{order:4}}";
    let exact = styled("<div style='--heroLayout:Row'><p id=p>x</p></div>", sheet);
    assert_eq!(style(&exact, "p").order, 4);
    let other = styled("<div style='--herolayout:row'><p id=p>x</p></div>", sheet);
    assert_eq!(style(&other, "p").order, 0);
}

#[test]
fn a_named_style_query_reads_the_nearest_container_with_that_name() {
    let sheet = "#card{container-name:card;--tone:warm}#inner{--tone:cold}\
                 @container card style(--tone: warm){p{order:5}}";
    let document = styled(
        "<div id=card><div id=inner><p id=p>x</p></div></div>",
        sheet,
    );
    assert_eq!(style(&document, "p").order, 5);
}

#[test]
fn an_inherited_custom_property_answers_a_style_query() {
    let document = styled(
        "<section style='--theme-variant:dark'><div><p id=p>x</p></div></section>",
        THEME,
    );
    assert_eq!(style(&document, "p").order, 2);
}

/** Notion's hero: a style condition and a width condition on one query. */
const HERO: &str = "#hero{container-type:inline-size;--hero-layout-row:1}\
                    @container style(--hero-layout-row:1) and (min-width:800px){p{order:6}}";

#[test]
fn a_mixed_query_reads_style_and_size_from_the_same_size_container() {
    let markup =
        |width: u32| format!("<div id=hero style='width:{width}px'><div><p id=p>x</p></div></div>");
    assert_eq!(style(&styled(&markup(1000), HERO), "p").order, 6);
    assert_eq!(style(&styled(&markup(500), HERO), "p").order, 0);
}

#[test]
fn height_queries_read_a_size_containers_content_height() {
    let sheet = "#c{container-type:size;width:300px;height:200px;padding:10px}\
                 @container (min-height:200px){p{order:1}}\
                 @container (min-height:201px){p{z-index:2}}";
    let document = styled("<div id=c><p id=p>x</p></div>", sheet);
    let p = style(&document, "p");
    assert_eq!(p.order, 1, "the padding sits outside the 200px content box");
    assert_eq!(p.z_index, None);
}

#[test]
fn orientation_and_aspect_ratio_read_both_axes() {
    let sheet = "#c{container-type:size;width:300px;height:200px}\
                 @container (orientation: landscape){p{order:1}}\
                 @container (min-aspect-ratio: 2/1){p{z-index:3}}";
    let document = styled("<div id=c><p id=p>x</p></div>", sheet);
    let p = style(&document, "p");
    assert_eq!(p.order, 1);
    assert_eq!(p.z_index, None, "3:2 is narrower than 2:1");
}

#[test]
fn a_height_query_on_an_inline_size_container_stays_undecided() {
    let document = styled(
        "<div id=c><a id=a href=/x>a</a></div>",
        "#c{container-type:inline-size;width:300px}\
         @container (min-height:1px){a{display:none}}",
    );
    let a = style(&document, "a");
    assert!(!a.hidden, "the rule does not apply");
    assert!(!a.interactive, "but what it could hide is masked");
}

#[test]
fn container_width_units_resolve_against_the_nearest_size_container() {
    let document = styled(
        "<div id=c><p id=p>x</p><p id=q>y</p></div>",
        "#c{container-type:inline-size;width:400px}\
         #p{width:50cqw}#q{width:calc(100cqi - 40px)}",
    );
    assert_eq!(style(&document, "p").width, Some(200.0));
    assert_eq!(style(&document, "q").width, Some(360.0));
}

#[test]
fn container_height_units_need_a_size_container() {
    let sheet = "#c{container-type:size;width:400px;height:300px}#i{container-type:inline-size}\
                 #p{width:10cqh}#q{width:10cqmin}";
    let document = styled(
        "<div id=c><div id=i><p id=p>x</p><p id=q>y</p></div></div>",
        sheet,
    );
    assert_eq!(
        style(&document, "p").width,
        Some(30.0),
        "the inline-size box is skipped"
    );
    assert_eq!(style(&document, "q").width, Some(30.0));
}

#[test]
fn container_units_without_a_container_use_the_viewport() {
    let document = styled("<p id=p>x</p>", "#p{width:10cqw}");
    let viewport = mars_eye::RenderContext::default().width;
    assert_eq!(style(&document, "p").width, Some(viewport / 10.0));
}

#[test]
fn hostile_style_queries_never_panic() {
    for prelude in [
        "style(",
        "style()",
        "style(--)",
        "style(:)",
        "style(--x: (((",
        "style(color: red)",
        "not style(--x)",
        "style(--x) or style(",
        "card style(--\u{e9}: \u{e9})",
    ] {
        let sheet = format!("@container {prelude} {{p{{order:1}}}}");
        styled("<div style='--x:1'><p id=p>x</p></div>", &sheet);
    }
}
