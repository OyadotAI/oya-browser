/** Background rules from the inline builder's pass: `background-attachment: fixed` positions an
 * image layer in the viewport, from the longhand and from the `background` shorthand; gradient
 * layers tile with their own geometry; corners can be elliptical; and the canvas takes no
 * background from a root or body without a box. */
use mars_eye::{DisplayBackground, compute_styles, parse, render};

/** The positioning area override of the first image layer in `markup`. */
fn area(markup: &str) -> Option<[f64; 4]> {
    let document = compute_styles(&parse(markup).unwrap(), &[]).unwrap();
    let items = render(&document, 800.0).unwrap().items;
    items
        .iter()
        .find_map(|item| match item.backgrounds.first() {
            Some(DisplayBackground::Image { area, .. }) => Some(*area),
            _ => None,
        })?
}

/** A fixed layer takes the viewport as its positioning area; a scrolling one keeps the box. */
#[test]
fn fixed_attachment_positions_in_the_viewport() {
    let fixed = "<div style='margin-top:50px;height:10px;background:url(a.png) fixed'></div>";
    assert_eq!(area(fixed).map(|a| [a[0], a[1]]), Some([0.0, 0.0]));
    let longhand =
        "<div style='height:10px;background-image:url(a.png);background-attachment:fixed'></div>";
    assert!(area(longhand).is_some());
    let scrolling = "<div style='height:10px;background:url(a.png)'></div>";
    assert_eq!(area(scrolling), None);
}

/** The first background layer of the first div in `markup`. */
fn layer(markup: &str) -> DisplayBackground {
    let document = compute_styles(&parse(markup).unwrap(), &[]).unwrap();
    let items = render(&document, 800.0).unwrap().items;
    items[0].backgrounds[0].clone()
}

/** A gradient fills its `background-size` tile at its position, spaced per `space`; one over
 * the whole border box records no tile. */
#[test]
fn gradient_layers_tile_like_images() {
    let spaced = layer(
        "<div style='width:106px;height:106px;background:linear-gradient(red,green) 0 0/32px 32px space'></div>",
    );
    let tile = spaced.tile().expect("a tile");
    assert_eq!(
        [tile.x, tile.y, tile.width, tile.height],
        [0.0, 0.0, 32.0, 32.0]
    );
    assert_eq!(tile.repeat, [true, true]);
    assert!((tile.gap[0] - 5.0).abs() < 1e-9);
    let whole =
        layer("<div style='width:10px;height:10px;background:linear-gradient(red,green)'></div>");
    assert!(whole.tile().is_none());
}

/** Each layer takes its own size, position and repeat from the comma lists, skipping none. */
#[test]
fn each_layer_takes_its_own_geometry() {
    let markup = "<div style='width:100px;height:100px;background:none, linear-gradient(red,red) 10px 0/20px 20px no-repeat, linear-gradient(blue,blue)'></div>";
    let document = compute_styles(&parse(markup).unwrap(), &[]).unwrap();
    let items = render(&document, 800.0).unwrap().items;
    let tile = items[0].backgrounds[0]
        .tile()
        .expect("first gradient's tile");
    assert_eq!(
        [tile.x, tile.y, tile.width, tile.height],
        [10.0, 0.0, 20.0, 20.0]
    );
    assert_eq!(tile.repeat, [false, false]);
    assert!(items[0].backgrounds[1].tile().is_none());
}

/** The first item of `markup` laid out alone. */
fn first(markup: &str) -> mars_eye::DisplayItem {
    let document = compute_styles(&parse(markup).unwrap(), &[]).unwrap();
    render(&document, 800.0).unwrap().items[0].clone()
}

/** A corner's two radii (a pair, a `/` list, or one percentage over a non-square border box)
 * make it elliptical; a negative one drops the declaration; circular corners record none. */
#[test]
fn elliptical_corner_radii() {
    let pair =
        first("<div style='width:200px;height:100px;border-top-left-radius:20px 10px'></div>");
    assert_eq!(
        (pair.radius[0], pair.radius_y.map(|y| y[0])),
        (20.0, Some(10.0))
    );
    let percent = first(
        "<div style='width:200px;height:100px;border:20px solid;border-top-left-radius:20%'></div>",
    );
    assert_eq!(
        (percent.radius[0], percent.radius_y.map(|y| y[0])),
        (48.0, Some(28.0))
    );
    let slash = first("<div style='width:100px;height:100px;border-radius:10px / 5px'></div>");
    assert_eq!(slash.radius_y, Some([5.0; 4]));
    let negative =
        first("<div style='width:100px;height:100px;border-top-left-radius:20px -5px'></div>");
    assert_eq!((negative.radius, negative.radius_y), ([0.0; 4], None));
    let circle = first("<div style='width:100px;height:100px;border-radius:10px'></div>");
    assert_eq!((circle.radius, circle.radius_y), ([10.0; 4], None));
}

/** Image layers also take their own list entries: the first kept image is the second
 * authored layer (after `none`), positioned and tiled as that layer says. */
#[test]
fn each_image_layer_takes_its_own_geometry() {
    let markup = "<div style='width:100px;height:100px;background:none, url(a.png) 5px 0 no-repeat, url(b.png)'></div>";
    let document = compute_styles(&parse(markup).unwrap(), &[]).unwrap();
    let items = render(&document, 800.0).unwrap().items;
    let positions: Vec<([f64; 4], [bool; 2])> = items[0]
        .backgrounds
        .iter()
        .filter_map(|layer| match layer {
            DisplayBackground::Image {
                position, repeat, ..
            } => Some((*position, *repeat)),
            _ => None,
        })
        .collect();
    assert_eq!(
        positions,
        vec![([5.0, 0.0, 0.0, 0.0], [false; 2]), ([0.0; 4], [true; 2])]
    );
}

/** The canvas background of `markup` styled by `sheet`. */
fn canvas(markup: &str, sheet: &str) -> Option<String> {
    let document = compute_styles(&parse(markup).unwrap(), &[sheet.to_string()]).unwrap();
    render(&document, 800.0).unwrap().canvas_background
}

/** A root or body with `display: none` (or a `display: contents` body) gives the canvas no
 * background; an ordinary body's background propagates. */
#[test]
fn boxless_root_and_body_do_not_propagate() {
    let white = Some("#ffffff".to_string());
    assert_eq!(
        canvas("<body></body>", "html { background: red; display: none }"),
        white
    );
    assert_eq!(
        canvas("<body></body>", "body { background: red; display: none }"),
        white
    );
    assert_eq!(
        canvas(
            "<body></body>",
            "body { background: red; display: contents }"
        ),
        white
    );
    assert_eq!(
        canvas("<body></body>", "body { background: red }"),
        Some("red".to_string())
    );
}
