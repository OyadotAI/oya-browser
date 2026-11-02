/** Round-four rules from the inline builder: a decorated block's text paints in the inline
 * phase through a text echo, the root's generated boxes lay out in a root box above and below
 * the body, every image layer (and the canvas's) has an image key of its own, and a `local`
 * background scrolls with its container's contents. */
use mars_eye::{DisplayBackground, DisplayList, compute_styles, parse, render};

/** The display list of `markup` styled by `sheet`. */
fn laid(markup: &str, sheet: &str) -> DisplayList {
    let document = compute_styles(&parse(markup).unwrap(), &[sheet.to_string()]).unwrap();
    render(&document, 800.0).unwrap()
}

/** A bordered block keeps its runs (for agents) but defers painting them to an echo placed
 * after a later sibling block's background; an undecorated one needs no echo. */
#[test]
fn decorated_block_text_paints_through_an_echo() {
    let list = laid(
        "<p class=a>text</p><p class=b></p>",
        "p { margin: 0; height: 5px } .a { border: 1px solid } .b { background: green }",
    );
    let items = &list.items;
    let source = items.iter().position(|item| item.text_deferred).unwrap();
    assert_eq!(items[source].lines[0].text, "text");
    let echo = items.iter().position(|item| item.text_echo).unwrap();
    let background = items
        .iter()
        .position(|item| item.background.as_deref() == Some("green"))
        .unwrap();
    assert!(source < background && background < echo);
    assert!(items[echo].lines.is_empty() && items[echo].local_id == items[source].local_id);
    let plain = laid("<p>text</p>", "");
    assert!(plain.items.iter().all(|item| !item.text_echo));
}

/** `html::before` lays out in the root's box above the body, whose margin it collapses with,
 * and `html::after` below the body. */
#[test]
fn root_generated_boxes_lay_out_around_the_body() {
    let list = laid(
        "<body><p>body</p></body>",
        "html::before { content: 'top'; display: block; margin: 8px } html::after { content: 'end'; display: block }\
         body { margin: 8px } p { margin: 0 }",
    );
    let y = |text: &str| {
        list.items
            .iter()
            .flat_map(|item| item.lines.iter())
            .find(|line| line.text == text)
            .unwrap()
            .y
    };
    assert!(y("top") < y("body") && y("body") < y("end"));
    let top = list
        .items
        .iter()
        .find(|item| item.tag == "::before")
        .unwrap();
    assert_eq!((top.x, top.y), (8.0, 8.0));
}

/** An element's second image layer, and the canvas's propagated image, carry keys of their
 * own that name their owner (or no element, for the canvas). */
#[test]
fn image_layers_have_keys_of_their_own() {
    let list = laid(
        "<div style='height:10px;background:url(a.png), url(b.png)'></div>",
        "body { background: url(c.png) }",
    );
    let keys: Vec<Option<usize>> = list.items[0]
        .backgrounds
        .iter()
        .map(|layer| match layer {
            DisplayBackground::Image { key, .. } => *key,
            _ => None,
        })
        .collect();
    let local = list.items[0].local_id;
    assert_eq!(keys, vec![None, Some(mars_eye::layer_key(local, 1))]);
    assert_eq!(mars_eye::background_owner(keys[1].unwrap()), Some(local));
    let canvas = list.canvas_layers.expect("canvas layers");
    let DisplayBackground::Image { key, .. } = &canvas.backgrounds[0] else {
        panic!("an image layer");
    };
    assert_eq!(*key, Some(mars_eye::canvas_key(0)));
    assert_eq!(mars_eye::background_owner(mars_eye::canvas_key(0)), None);
}

/** The positioning area of the first image layer of `#s`, scrolled by (40, 60), with the
 * given `background-attachment`. */
fn scrolled_area(attachment: &str) -> Option<[f64; 4]> {
    let markup = format!(
        "<div id=s style='overflow:hidden;width:200px;height:300px;\
         background:url(a.png) {attachment} no-repeat'><div style='width:250px;height:370px'></div></div>"
    );
    let document = compute_styles(&parse(&markup).unwrap(), &["body{margin:0}".into()]).unwrap();
    let local = document
        .nodes
        .iter()
        .find(|n| n.html_id == "s")
        .unwrap()
        .local_id;
    let offsets = mars_eye::ScrollOffsets::from([(local, [40.0, 60.0])]);
    let context = mars_eye::RenderContext {
        width: 800.0,
        height: 600.0,
        ..Default::default()
    };
    let list = mars_eye::render_scrolled(&document, context, &offsets).unwrap();
    let item = list
        .items
        .into_iter()
        .find(|item| item.local_id == local)
        .unwrap();
    match &item.backgrounds[0] {
        DisplayBackground::Image { area, .. } => *area,
        _ => panic!("an image layer"),
    }
}

/** A scroll container's `local` image layers are positioned against its scrollable overflow at
 * its scroll offset; a `scroll` one keeps the box. */
#[test]
fn local_background_scrolls_with_the_contents() {
    assert_eq!(scrolled_area("local"), Some([-40.0, -60.0, 250.0, 370.0]));
    assert_eq!(scrolled_area("scroll"), None);
}
