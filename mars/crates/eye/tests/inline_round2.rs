/** Round-two rules from the inline builder: the body's generated boxes, the canvas's propagated
 * gradient layers, `calc()` in media queries, case-insensitive units, inherited clips and
 * origin-box image sizes, typed `attr()` checks and `content: attr()` fallbacks. */
use mars_eye::{DisplayList, compute_styles, parse, render};

/** The display list of `markup` styled by `sheet`. */
fn laid(markup: &str, sheet: &str) -> DisplayList {
    let document = compute_styles(&parse(markup).unwrap(), &[sheet.to_string()]).unwrap();
    render(&document, 800.0).unwrap()
}

/** `body::before` and `body::after` lay out first and last in the body's flow. */
#[test]
fn body_generated_boxes_lay_out_around_its_content() {
    let list = laid(
        "<body><p>middle</p></body>",
        "body::before { content: 'first'; display: block } body::after { content: 'last' }",
    );
    let texts: Vec<&str> = list
        .items
        .iter()
        .flat_map(|item| item.lines.iter().map(|line| line.text.as_str()))
        .collect();
    assert_eq!(texts, vec!["first", "middle", "last"]);
}

/** A root gradient propagates to the canvas: positioned in the root's box and tiling across
 * the whole document; a plain color stays the canvas color alone. */
#[test]
fn root_gradients_propagate_to_the_canvas() {
    let list = laid(
        "<body></body>",
        "html { background: linear-gradient(red, blue); margin: 50px; height: 100px }",
    );
    let canvas = list.canvas_layers.expect("canvas layers");
    let tile = canvas.backgrounds[0].tile().expect("a tile");
    assert_eq!((tile.x, tile.y, tile.repeat), (50.0, 50.0, [true; 2]));
    assert!(canvas.height >= list.height);
    assert!(
        laid("<body></body>", "html { background: red }")
            .canvas_layers
            .is_none()
    );
}

/** Media features take `calc()` lengths, and length units ignore ASCII case. */
#[test]
fn media_calc_and_unit_case() {
    let list = laid(
        "<div class=a></div>",
        "@media (min-width: calc(100px)) { .a { height: 10PX } }",
    );
    assert_eq!(list.items[0].height, 10.0);
}

/** A typed `attr()` whose attribute is not of its type takes the fallback, even in a shorthand
 * that could have read the bad value as something else. */
#[test]
fn typed_attr_rejects_values_of_another_type() {
    let list = laid(
        "<div data-c=qqffuutt></div>",
        "div { height: 10px; background: attr(data-c type(<color>), green) }",
    );
    assert_eq!(list.items[0].background.as_deref(), Some("green"));
}

/** `content: attr(name, "fallback")` shows the attribute, else the fallback string (case kept);
 * a missing attribute with a non-string fallback generates nothing. */
#[test]
fn content_attr_falls_back_to_its_string() {
    let list = laid(
        "<p id=a></p><p id=b data-x=Here></p><p id=c></p>",
        "#a::after, #b::after { content: attr(data-x, \"Fallback\") } #c::after { content: attr(data-x, bad) }",
    );
    let texts: Vec<&str> = list
        .items
        .iter()
        .flat_map(|item| item.lines.iter().map(|line| line.text.as_str()))
        .collect();
    assert_eq!(texts, vec!["Fallback", "Here"]);
}

/** `background-clip: inherit` takes the parent's clip, and an image's percentage size
 * resolves against its `background-origin` box (here the content box). */
#[test]
fn clip_inherits_and_image_sizes_follow_the_origin_box() {
    let list = laid(
        "<div class=p><div class=c></div></div>",
        ".p { background-clip: content-box } .c { background-clip: inherit; background: url(a.png) no-repeat; background-clip: inherit; background-origin: content-box; background-size: 100% 100%; padding: 10px; border: 5px solid; width: 40px; height: 20px }",
    );
    let child = &list.items[1];
    assert_eq!(child.background_clip, [15.0; 4]);
    let Some(mars_eye::DisplayBackground::Image { size, .. }) = child.backgrounds.first() else {
        panic!("an image layer");
    };
    assert_eq!(
        *size,
        mars_eye::ImageSize::Fixed {
            width: Some(40.0),
            height: Some(20.0)
        }
    );
}
