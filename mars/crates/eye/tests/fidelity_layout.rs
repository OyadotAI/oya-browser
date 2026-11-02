/**
 * Layout fidelity pinned on what Google's Gmail page and LinkedIn's guest home page need from the
 * cascade and the flex and grid algorithms: `:defined`, `span calc()`, flex auto margins and a
 * column's `min-height`. Each case is the smallest markup that showed the same defect.
 */
use mars_eye::{
    DisplayItem, DisplayList, Document, RenderContext, compute_styles_with_context, parse,
    render_with_context,
};

/** The viewport the pages were compared at. */
fn viewport() -> RenderContext {
    RenderContext {
        width: 1100.0,
        height: 751.0,
        ..RenderContext::default()
    }
}

/** Styles `body` with `css` in the viewport, then lays it out. */
fn layout(body: &str, css: &str) -> (Document, DisplayList) {
    let document = parse(body).unwrap();
    let css = format!("body {{ margin: 0 }} {css}");
    let styled = compute_styles_with_context(&document, &[css], viewport()).unwrap();
    let display = render_with_context(&styled, viewport()).unwrap();
    (styled, display)
}

/** The first item painted for the element with `id`. */
fn find<'a>(document: &Document, display: &'a DisplayList, id: &str) -> &'a DisplayItem {
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

/** Whether the element with `id` is hidden from paint and agents. */
fn hidden(document: &Document, id: &str) -> bool {
    document
        .nodes
        .iter()
        .find(|node| node.html_id == id)
        .unwrap()
        .hidden()
}

/** Equal within layout rounding; the values are in the failure message. */
fn close(a: f64, b: f64) -> bool {
    assert!((a - b).abs() < 0.01, "{a} vs {b}");
    true
}

/** Gmail's `gws-action-bar:not(:defined) { visibility: hidden }` hides the bar's light DOM until
a script upgrades the element; without the pseudo-class the rule was dropped and the bar's
buttons rendered a second time under the header. */
#[test]
fn not_defined_hides_an_unupgraded_custom_element() {
    let (document, _) = layout(
        "<x-bar id=bar><details><summary id=s>Create an account</summary></details></x-bar>\
         <x-up id=up><template shadowrootmode=open><slot></slot></template>Up</x-up><p id=p>ok</p>",
        "x-bar:not(:defined), x-up:not(:defined), p:not(:defined) { visibility: hidden }",
    );
    assert!(hidden(&document, "bar"));
    assert!(hidden(&document, "s"), "hidden content is not actionable");
    assert!(
        !hidden(&document, "up"),
        "an upgraded host with a shadow root is defined"
    );
    assert!(
        !hidden(&document, "p"),
        "built-in elements are always defined"
    );
}

/** Gmail's grid columns place with `grid-column-end: span calc(var(--end) - var(--start) + 1)`;
when the number inside `span` could not be a `calc()`, every column collapsed to one track. */
#[test]
fn grid_span_takes_a_calc_number() {
    let (document, display) = layout(
        "<div class=g><div id=c class=c style='--s:3;--e:10'>Gemini in Gmail</div></div>",
        ".g { display: grid; grid-template-columns: repeat(12, 1fr); column-gap: 24px; \
         padding: 0 24px; box-sizing: border-box } \
         .c { grid-column-start: var(--s); grid-column-end: span calc(var(--e) - var(--s) + 1) }",
    );
    let column = find(&document, &display, "c");
    let track = (1100.0 - 48.0 - 11.0 * 24.0) / 12.0;
    assert!(close(column.x, 24.0 + 2.0 * (track + 24.0)));
    assert!(close(column.width, 8.0 * track + 7.0 * 24.0));
}

/** LinkedIn's nav list carries `margin-left: auto` to push itself to the sign-in buttons; a
main-axis auto margin takes the line's free space before `justify-content` sees any. */
#[test]
fn flex_auto_margin_absorbs_main_axis_free_space() {
    let (document, display) = layout(
        "<nav id=n><a id=logo>Logo</a><ul id=menu>Menu</ul><div id=cta>Join</div></nav>",
        "nav { display: flex; width: 1000px; justify-content: center } \
         a, ul, div { width: 100px; margin: 0; padding: 0 } ul { margin-left: auto }",
    );
    let menu = find(&document, &display, "menu");
    let cta = find(&document, &display, "cta");
    assert!(close(menu.x, 800.0));
    assert!(close(cta.x, 900.0));
}

/** Cross-axis auto margins center an item in its line and win over `align-items: stretch`. */
#[test]
fn flex_cross_auto_margins_center_an_item() {
    let (document, display) = layout(
        "<div id=row><p id=p>x</p></div>",
        "div { display: flex; height: 100px } p { height: 20px; margin: auto 0 }",
    );
    let item = find(&document, &display, "p");
    assert!(close(item.y, 40.0));
    assert!(close(item.height, 20.0));
}

/** A column flex container with `min-height` lays its items out in that height, so LinkedIn's
nav links center their icon and label (`justify-content: center; min-height: 52px`). */
#[test]
fn flex_column_min_height_gives_justification_its_space() {
    let (document, display) = layout(
        "<a id=link><i id=icon></i><span id=label>Jobs</span></a>",
        "a { display: flex; flex-direction: column; justify-content: center; align-items: center; \
         min-height: 52px; line-height: 15px; font-size: 12px } \
         i { display: block; height: 24px; width: 24px }",
    );
    let icon = find(&document, &display, "icon");
    let label = find(&document, &display, "label");
    assert!(close(icon.y, (52.0 - 24.0 - 15.0) / 2.0));
    assert!(close(label.y, icon.y + 24.0));
}
