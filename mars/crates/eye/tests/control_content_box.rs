/** Text controls carry their content box, so the native editor can inset typed text by the
page's padding and border. Google's search box centers its one line this way. */
use mars_eye::{compute_styles, parse, render};

/** Google's search textarea rules: `textarea.gLFyf` outranks the later `.gLFyf{padding:0}`. */
const GOOGLE: &str = "textarea.gLFyf,.svBvU{line-height:22px;border-bottom:8px solid transparent;padding-top:14px}\
    .gLFyf{width:100%;background-color:transparent;border:none;margin:0;padding:0;display:flex}";

/** A padded textarea's display item starts its content below the padding. */
#[test]
fn a_padded_textarea_carries_its_content_box() {
    let markup = "<div style='display:flex;height:50px'><textarea class=gLFyf id=t rows=1></textarea></div>";
    let document = compute_styles(&parse(markup).unwrap(), &[GOOGLE.to_owned()]).unwrap();
    let display = render(&document, 800.0).unwrap();
    let item = display.items.iter().find(|item| item.tag == "textarea").unwrap();
    let [_, y, _, height] = item.content_box.expect("content box");
    assert_eq!(y - item.y, 14.0);
    assert_eq!(height, item.height - 14.0 - 8.0);
}

/** A control with no padding or border needs no inset. */
#[test]
fn an_unpadded_input_has_no_content_box() {
    let markup = "<input id=t style='padding:0;border:0'>";
    let document = compute_styles(&parse(markup).unwrap(), &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    let item = display.items.iter().find(|item| item.tag == "input").unwrap();
    assert_eq!(item.content_box, None);
}
