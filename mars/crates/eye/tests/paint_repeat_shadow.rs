/** Round-4 paint ports: `background-repeat: space | round` per axis reaches the display list,
 * and a `currentcolor` box shadow inherited by a child paints the child's own color. */
use mars_eye::{DisplayBackground, compute_styles, parse, render};

/** The display items of `markup`. */
fn items(markup: &str) -> Vec<mars_eye::DisplayItem> {
    let document = compute_styles(&parse(markup).unwrap(), &[]).unwrap();
    render(&document, 800.0).unwrap().items
}

/** `space round` sets each axis on its own and both still repeat for older painters. */
#[test]
fn space_and_round_reach_the_layer() {
    let items = items(
        "<div style='width:100px;height:100px;background:url(a.png);background-repeat:space round'></div>",
    );
    let layer = items
        .iter()
        .find_map(|item| item.backgrounds.first())
        .unwrap();
    let DisplayBackground::Image {
        repeat,
        space,
        round,
        ..
    } = layer
    else {
        panic!("image layer expected: {layer:?}")
    };
    assert_eq!(
        (*repeat, *space, *round),
        ([true; 2], [true, false], [false, true])
    );
}

/** `box-shadow: inherit` of a `currentcolor` shadow paints with the inheriting box's color. */
#[test]
fn inherited_currentcolor_shadow_uses_own_color() {
    let items = items(
        "<div style='color:transparent;box-shadow:1px 1px currentcolor'><p style='color:#00ff00;box-shadow:inherit'>x</p></div>",
    );
    let p = items.iter().find(|item| item.tag == "p").unwrap();
    assert_eq!(p.shadows[0].color, "#00ff00");
}
