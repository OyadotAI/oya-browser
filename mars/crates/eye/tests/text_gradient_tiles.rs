/** Gradient background layers placed by `background-size`, `-position` and `-repeat`, which the
 * css-text white-space tests use to paint red squares that text must cover: the tile, its
 * repeat, `space` and `round`, and each layer's own entry of the comma-separated lists. */
use mars_eye::{DisplayItem, GradientTile, compute_styles, parse, render};

/** The page width the cases lay out at. */
const VIEWPORT: f64 = 800.0;

/** The `#t` element's display item, with `style` on it. */
fn target(style: &str) -> DisplayItem {
    let markup = format!("<body style='margin:0'><div id=t style='{style}'></div>");
    let document = compute_styles(&parse(&markup).unwrap(), &[]).unwrap();
    let local = document
        .nodes
        .iter()
        .find(|n| n.html_id == "t")
        .unwrap()
        .local_id;
    let display = render(&document, VIEWPORT).unwrap();
    display
        .items
        .into_iter()
        .find(|i| i.local_id == local)
        .unwrap()
}
/** The tile of each background layer, bottom layer last. */
fn tiles(style: &str) -> Vec<Option<GradientTile>> {
    target(style).backgrounds.iter().map(|b| b.tile()).collect()
}

/** A sized, positioned, non-repeating gradient paints one tile where the shorthand puts it. */
#[test]
fn a_sized_gradient_paints_one_placed_tile() {
    let found = tiles(
        "width:80px;height:40px;background:linear-gradient(red,red) 10px 5px/20px 30px no-repeat",
    );
    let tile = found[0].unwrap();
    assert_eq!(
        (tile.x, tile.y, tile.width, tile.height),
        (10.0, 5.0, 20.0, 30.0)
    );
    assert_eq!(tile.repeat, [false, false]);
    assert_eq!(tile.local(15.0, 10.0), Some((5.0, 5.0)));
    assert_eq!(tile.local(50.0, 10.0), None);
}

/** A repeating tile maps every point of the box into the tile it falls in. */
#[test]
fn a_repeating_tile_wraps() {
    let found = tiles("width:80px;height:40px;background:linear-gradient(red,red) 0 0/20px 20px");
    let tile = found[0].unwrap();
    assert_eq!(tile.local(45.0, 25.0), Some((5.0, 5.0)));
    assert_eq!(tile.origins(80.0, 40.0).len(), 8);
}

/** The default gradient over a borderless box keeps filling it as one gradient. */
#[test]
fn an_unplaced_gradient_fills_the_box() {
    assert_eq!(
        tiles("width:80px;height:40px;background:linear-gradient(red,red)"),
        [None]
    );
}

/** Percentages position the tile in the room the area leaves around it. */
#[test]
fn percentages_position_in_the_leftover_room() {
    let found = tiles(
        "width:100px;height:40px;background:linear-gradient(red,red) 50% 0/20px 20px no-repeat",
    );
    assert_eq!(found[0].unwrap().x, 40.0);
}

/** `round` rescales the tile so whole tiles span the area. */
#[test]
fn round_fits_whole_tiles() {
    let found =
        tiles("width:100px;height:40px;background:linear-gradient(red,red) 0 0/30px 40px round");
    assert_eq!(found[0].unwrap().width, 100.0 / 3.0);
}

/** `space` spreads whole tiles with the leftover between them, and a lone tile does not repeat. */
#[test]
fn space_spreads_whole_tiles() {
    let found =
        tiles("width:100px;height:40px;background:linear-gradient(red,red) 0 0/30px 40px space");
    let tile = found[0].unwrap();
    assert_eq!(tile.gap[0], 5.0);
    let lone =
        tiles("width:50px;height:40px;background:linear-gradient(red,red) 5px 0/30px 40px space");
    assert!(!lone[0].unwrap().repeat[0]);
}

/** Each layer takes its own entry of the comma-separated geometry lists. */
#[test]
fn each_layer_takes_its_own_geometry() {
    let found = tiles(
        "width:100px;height:40px;background:linear-gradient(red,red) 10px 0/10px 10px no-repeat,\
         linear-gradient(blue,blue) 50px 0/20px 20px no-repeat",
    );
    let (first, second) = (found[0].unwrap(), found[1].unwrap());
    assert_eq!((first.x, first.width), (10.0, 10.0));
    assert_eq!((second.x, second.width), (50.0, 20.0));
}

/** The positioning area is the padding box: a bordered box's tile starts inside the border. */
#[test]
fn tiles_start_at_the_padding_box() {
    let found = tiles(
        "width:100px;height:40px;border:5px solid;background:linear-gradient(red,red) 0 0/10px 10px no-repeat",
    );
    assert_eq!((found[0].unwrap().x, found[0].unwrap().y), (5.0, 5.0));
}
