/** Scroll containers paint their contents at the host's scroll offset without relaying them out,
 * and `width: stretch` (`-webkit-fill-available`) fills the containing block. Both hold up
 * LinkedIn's app-shell variant: a viewport-high grid whose `<main>` is the only scroller, and a
 * header row sized with `-webkit-fill-available`. */
use mars_eye::{
    DisplayItem, DisplayList, RenderContext, ScrollOffsets, StyleSource,
    compute_styles_with_context, parse, render_scrolled,
};

/** The document's own styles, laid out at 800 by 600 with `offsets` applied. */
fn layout(markup: &str, offsets: &[(usize, [f64; 2])]) -> (DisplayList, Vec<usize>) {
    let parsed = parse(markup).unwrap();
    let sheets = parsed
        .styles
        .iter()
        .filter_map(|source| match source {
            StyleSource::Inline(css) => Some(css.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let context = RenderContext {
        width: 800.0,
        height: 600.0,
        ..RenderContext::default()
    };
    let document = compute_styles_with_context(&parsed, &sheets, context).unwrap();
    let ids = document.nodes.iter().map(|node| node.local_id).collect();
    let offsets = offsets.iter().copied().collect::<ScrollOffsets>();
    (render_scrolled(&document, context, &offsets).unwrap(), ids)
}
/** The first painted item with this tag. */
fn tagged<'a>(display: &'a DisplayList, tag: &str) -> &'a DisplayItem {
    display
        .items
        .iter()
        .find(|item| item.tag == tag)
        .unwrap_or_else(|| panic!("{tag} was not painted"))
}
/** A 300px-high scroller (overflow-x hidden, overflow-y scroll, 10px padding) over 1000px of
 * content, with a fixed box inside it. */
const SHELL: &str = "<style>body{margin:0}main{display:block;height:300px;padding:10px;\
    overflow-x:hidden;overflow-y:scroll}section{display:block;height:1000px}\
    aside{display:block;position:fixed;top:0;left:0;width:10px;height:10px}</style>\
    <main><section></section><aside></aside></main>";
/** The local identity of the first node with this tag. */
fn id_of(markup: &str, tag: &str) -> usize {
    let parsed = parse(markup).unwrap();
    parsed
        .nodes
        .iter()
        .find(|node| node.tag == tag)
        .unwrap()
        .local_id
}

/** A container reports its scroll range: content plus end padding, less the padding box. */
#[test]
fn a_scroll_container_reports_its_range_and_which_axes_a_wheel_scrolls() {
    let (display, _) = layout(SHELL, &[]);
    let main = id_of(SHELL, "main");
    let area = display
        .scrollers
        .iter()
        .find(|area| area.local_id == main)
        .expect("main scrolls");
    assert_eq!((area.y, area.height), (0.0, 320.0), "{area:?}");
    assert_eq!(area.max_y, 1000.0 + 20.0 - 320.0, "{area:?}");
    assert!(area.user_y && !area.user_x, "{area:?}");
}

/** An offset moves the contents up, never the container, and never a fixed box. */
#[test]
fn an_offset_paints_contents_scrolled_but_not_the_container_or_fixed_boxes() {
    let main = id_of(SHELL, "main");
    let (display, _) = layout(SHELL, &[(main, [0.0, 250.0])]);
    assert_eq!(tagged(&display, "main").y, 0.0);
    assert_eq!(tagged(&display, "section").y, 10.0 - 250.0);
    assert_eq!(tagged(&display, "aside").y, 0.0);
    let clip = tagged(&display, "section").clip.expect("clipped to main");
    assert_eq!((clip.y, clip.height), (0.0, 320.0));
    assert_eq!(display.scrollers[0].scroll_y, 250.0);
}

/** Offsets past the range clamp to it, as Blink's `ClampScrollOffset` does. */
#[test]
fn an_offset_past_the_end_clamps_to_the_range() {
    let main = id_of(SHELL, "main");
    let (display, _) = layout(SHELL, &[(main, [40.0, 9999.0])]);
    assert_eq!(tagged(&display, "section").y, 10.0 - 700.0);
    assert_eq!(tagged(&display, "section").x, 10.0);
}

/** `width: -webkit-fill-available` on a flex item fills the flex container less its margins,
 * whatever `box-sizing` says, and a later unsupported `-moz-available` leaves it in force. */
#[test]
fn fill_available_width_fills_the_container_less_margins() {
    let markup = "<style>body{margin:0}.row{display:flex;width:600px}\
        p{display:flex;margin:0 10px;padding:0 5px;width:-webkit-fill-available;\
        width:-moz-available}b{display:block;width:50px}</style>\
        <div class=row><p><b></b></p></div>";
    let (display, _) = layout(markup, &[]);
    let p = tagged(&display, "p");
    assert_eq!((p.x, p.width), (10.0, 580.0), "{p:?}");
}

/** `width: stretch` is the standard spelling of the same size. */
#[test]
fn stretch_width_fills_a_block_container_in_border_box_sizing() {
    let markup = "<style>body{margin:0}div{width:400px}span{display:inline-block;\
        box-sizing:border-box;padding:0 20px;width:stretch}</style><div><span></span></div>";
    let (display, _) = layout(markup, &[]);
    assert_eq!(tagged(&display, "span").width, 400.0);
}

/** An absolute `::after` of a static element takes its containing block from the nearest
 * positioned ancestor, as Blink does, instead of being dropped (LinkedIn's full-bleed
 * "Post your job" band is `::after { position: absolute; height: 100%; width: 100vw }` on a
 * static flex column inside a relative section). */
#[test]
fn an_absolute_after_of_a_static_element_fills_the_positioned_ancestor() {
    let markup = "<style>body{margin:0}section{position:relative;height:200px}\
        div{display:flex;flex-direction:column;padding:20px}div::after{content:'';\
        position:absolute;top:0;right:0;width:100vw;height:100%;z-index:-2;\
        background:#f1ece5}</style><section><div><h2>x</h2></div></section>";
    let (display, _) = layout(markup, &[]);
    let band = tagged(&display, "::after");
    assert_eq!(
        (band.x, band.y, band.width, band.height),
        (0.0, 0.0, 800.0, 200.0)
    );
    assert_eq!(band.background.as_deref(), Some("#f1ece5"));
}

/** An element that paints no background image lists its `::after` image under its identity,
 * so the host fetches it (LinkedIn's lazy-loaded "Join your colleagues" scene). */
#[test]
fn a_generated_background_image_is_listed_under_its_element() {
    let markup = "<style>div::after{content:'';position:absolute;inset:0;\
        background-image:url(/scene.png)}</style><div id=d>x</div>";
    let parsed = parse(markup).unwrap();
    let sheets = parsed
        .styles
        .iter()
        .filter_map(|source| match source {
            StyleSource::Inline(css) => Some(css.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let document = compute_styles_with_context(&parsed, &sheets, RenderContext::default()).unwrap();
    let local = document
        .nodes
        .iter()
        .find(|n| n.html_id == "d")
        .unwrap()
        .local_id;
    assert_eq!(
        mars_eye::background_urls(&document, "https://site.example/"),
        [(local, "https://site.example/scene.png".to_string())]
    );
}

/** A percentage `min-width` floors a flex item's shrink at that share of the container
 * (LinkedIn's `min-w-[50%] w-[50%]` column beside an image wider than the rest of the row). */
#[test]
fn a_percentage_min_width_stops_a_flex_item_shrinking_below_it() {
    let markup = "<style>body{margin:0}.row{display:flex;width:600px}\
        p{margin:0;width:50%;min-width:50%}i{display:block;flex:none;width:500px}</style>\
        <div class=row><p>x</p><i></i></div>";
    let (display, _) = layout(markup, &[]);
    assert_eq!(tagged(&display, "p").width, 300.0);
    assert_eq!(tagged(&display, "i").x, 300.0);
}

/** `width: fit-content` keeps a flex column's item at its content width instead of stretching
 * it across the column (LinkedIn's "Show more" pill). */
#[test]
fn a_fit_content_item_is_not_stretched_in_a_flex_column() {
    let markup = "<style>body{margin:0}.col{display:flex;flex-direction:column;width:600px}\
        button{width:fit-content;padding:0;border:0}span{display:block;width:120px}</style>\
        <div class=col><button><span></span></button></div>";
    let (display, _) = layout(markup, &[]);
    assert_eq!(tagged(&display, "button").width, 120.0);
}

/** A centered flex item taller than its line overflows both edges equally, as Blink's unsafe
 * default alignment does, instead of hanging from the line's top. */
#[test]
fn a_centered_item_taller_than_its_row_overflows_both_edges() {
    let markup = "<style>body{margin:0}.row{display:flex;height:720px;margin-top:200px}\
        i{display:block;align-self:center;width:10px;height:840px}</style>\
        <div class=row><i></i></div>";
    let (display, _) = layout(markup, &[]);
    assert_eq!(tagged(&display, "i").y, 200.0 - 60.0);
}

/** A button sizes by its border box and resets an inherited line height to `normal`, as
 * html.css's `box-sizing: border-box` and `font: -webkit-small-control` do; LinkedIn's
 * `min-height: 32px; padding: 7px` "Show more" pill is 32px tall, not 46px. */
#[test]
fn a_button_is_border_box_sized_with_a_normal_line_height() {
    let markup = "<style>body{margin:0;line-height:3}div{display:flex;flex-direction:column}\
        button{display:block;min-height:32px;padding:7px 16px;border:0;font-size:14px}</style>\
        <div><button>x</button></div>";
    let (display, _) = layout(markup, &[]);
    assert_eq!(tagged(&display, "button").height, 32.0);
    let markup = "<style>body{margin:0;line-height:3}div{display:flex;flex-direction:column}\
        button{display:block;padding:0;border:0;font-size:14px}</style>\
        <div><button>Show more</button></div>";
    let (display, _) = layout(markup, &[]);
    assert!(tagged(&display, "button").height < 20.0);
}

/** A wheel goes to the innermost scroller under the point that can still move that way, and to
 * the viewport (None) once that scroller is at its end, as Blink's scroll chaining does. */
#[test]
fn a_wheel_targets_the_scroller_under_it_until_it_reaches_its_end() {
    let main = id_of(SHELL, "main");
    let (display, _) = layout(SHELL, &[]);
    assert_eq!(
        mars_eye::wheel_target(&display.scrollers, 100.0, 100.0, [0.0, 50.0]),
        Some(main)
    );
    assert_eq!(
        mars_eye::wheel_target(&display.scrollers, 100.0, 100.0, [0.0, -50.0]),
        None
    );
    assert_eq!(
        mars_eye::wheel_target(&display.scrollers, 100.0, 500.0, [0.0, 50.0]),
        None
    );
    let (display, _) = layout(SHELL, &[(main, [0.0, 9999.0])]);
    assert_eq!(
        mars_eye::wheel_target(&display.scrollers, 100.0, 100.0, [0.0, 50.0]),
        None
    );
}

/** A sticky header and everything in it carry the pin painters move them by: at scroll 0 it sits
 * where layout put it, and past its start it stays at its `top` inset until its containing block
 * runs out (LinkedIn's `sticky top-0` navigation). A fixed box moves with every scroll. */
#[test]
fn sticky_and_fixed_boxes_carry_their_viewport_pins() {
    let markup = "<style>body{margin:0}header{display:block;position:sticky;top:0;height:50px}\
        main{display:block;height:2000px}aside{display:block;position:fixed;top:0}</style>\
        <div><p style='margin:0;height:100px'>a</p><header><a href=/x>x</a></header><main></main>\
        <aside>f</aside></div>";
    let (display, _) = layout(markup, &[]);
    let pin = tagged(&display, "header")
        .pin
        .expect("sticky header pinned");
    assert_eq!(tagged(&display, "a").pin, Some(pin));
    assert_eq!(pin.shift(0.0), 0.0);
    assert_eq!(pin.shift(60.0), 0.0);
    assert_eq!(pin.shift(400.0), 300.0);
    assert_eq!(pin.shift(5000.0), 2000.0);
    assert_eq!(
        tagged(&display, "aside").pin.map(|pin| pin.shift(700.0)),
        Some(700.0)
    );
    assert_eq!(tagged(&display, "main").pin, None);
}

/** An image whose source has not arrived keeps its authored height, so a lazy loader's
 * in-viewport test sees it where Chromium does (LinkedIn's `h-[840px]` brand image, centered
 * 60px above its 720px row). */
#[test]
fn an_image_without_a_source_keeps_its_authored_height() {
    let markup = "<style>body{margin:0}.row{display:flex;height:720px}img{height:840px;\
        align-self:center}</style><div class=row><img alt=''></div><img>";
    let (display, _) = layout(markup, &[]);
    let images = display
        .items
        .iter()
        .filter(|item| item.tag == "img")
        .map(|item| (item.y, item.height))
        .collect::<Vec<_>>();
    assert_eq!(images, [(-60.0, 840.0), (720.0, 840.0)]);
}
