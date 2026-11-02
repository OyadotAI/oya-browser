/** Heights, floats, inline-blocks, vertical alignment and inline box edges in the display list. */
use mars_eye::{DisplayItem, DisplayList, Document, compute_styles, parse, render};

/** A styled document and its display list at an 800 point viewport. */
fn layout(markup: &str) -> (Document, DisplayList) {
    let document = compute_styles(&parse(markup).unwrap(), &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    (document, display)
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
/** Equal within layout rounding. */
fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.01
}

/** A pixel height sets the border box, plus padding and border under content-box sizing. */
#[test]
fn height_sets_content_box_height() {
    let (document, display) =
        layout("<div id=a style='height:50px;padding:5px;border:2px solid'>x</div>");
    assert!(close(find(&document, &display, "a").height, 64.0));
}

/** Under border-box sizing the authored height already includes padding and border. */
#[test]
fn border_box_height_includes_edges() {
    let (document, display) = layout(
        "<div id=a style='box-sizing:border-box;height:50px;padding:5px;border:2px solid'>x</div>",
    );
    assert!(close(find(&document, &display, "a").height, 50.0));
}

/** min-height grows a short box and max-height caps a tall one. */
#[test]
fn min_and_max_height_clamp() {
    let (document, display) = layout(
        "<div id=a style='min-height:80px'>x</div><div id=b style='max-height:10px'>x<br>y<br>z</div>",
    );
    assert!(close(find(&document, &display, "a").height, 80.0));
    assert!(close(find(&document, &display, "b").height, 10.0));
}

/** A percentage height resolves against a definite parent height and is auto otherwise. */
#[test]
fn percent_height_needs_a_definite_parent() {
    let (document, display) = layout(
        "<div style='height:200px'><div id=a style='height:50%'>x</div></div>\
         <div><div id=b style='height:50%'>x</div></div>",
    );
    assert!(close(find(&document, &display, "a").height, 100.0));
    assert!(find(&document, &display, "b").height < 50.0);
}

/** A following block moves down by a box's authored height, not its content. */
#[test]
fn height_moves_following_blocks() {
    let (document, display) = layout("<div id=a style='height:120px'></div><div id=b>after</div>");
    let a = find(&document, &display, "a");
    assert!(close(find(&document, &display, "b").y, a.y + 120.0));
}

/** A left float sits at the left edge and following text starts past it. */
#[test]
fn left_float_shortens_lines() {
    let (document, display) = layout(
        "<div style='width:400px'><div id=f style='float:left;width:100px;height:60px'></div>\
         <span id=t>Some text beside the float</span></div>",
    );
    let float = find(&document, &display, "f");
    let text = find(&document, &display, "t");
    assert!(close(float.x, 8.0));
    assert!(text.lines[0].x >= float.x + float.width);
    assert!(close(text.lines[0].y, float.y));
}

/** A right float hugs the right edge of its container. */
#[test]
fn right_float_hugs_right_edge() {
    let (document, display) = layout(
        "<div id=c style='width:400px'><div id=f style='float:right;width:100px;height:20px'></div>x</div>",
    );
    let container = find(&document, &display, "c");
    let float = find(&document, &display, "f");
    assert!(close(float.x + float.width, container.x + container.width));
}

/** Two left floats stack side by side. */
#[test]
fn floats_stack_horizontally() {
    let (document, display) = layout(
        "<div style='width:400px'><div id=a style='float:left;width:100px;height:20px'></div>\
         <div id=b style='float:left;width:100px;height:20px'></div></div>",
    );
    let a = find(&document, &display, "a");
    let b = find(&document, &display, "b");
    assert!(close(b.x, a.x + a.width));
    assert!(close(a.y, b.y));
}

/** clear moves a block below earlier floats. */
#[test]
fn clear_moves_below_floats() {
    let (document, display) = layout(
        "<div id=f style='float:left;width:100px;height:60px'></div><div id=c style='clear:both'>x</div>",
    );
    let float = find(&document, &display, "f");
    assert!(find(&document, &display, "c").y >= float.y + float.height);
}

/** A block formatting context root grows to contain its floats. */
#[test]
fn overflow_hidden_contains_floats() {
    let (document, display) = layout(
        "<div id=c style='overflow:hidden'><div style='float:left;width:50px;height:90px'></div></div>",
    );
    assert!(close(find(&document, &display, "c").height, 90.0));
}

/** An inline-block shrinks to its content and sits on the line with the text after it. */
#[test]
fn inline_block_is_atomic_on_the_line() {
    let (document, display) = layout(
        "<div><span id=b style='display:inline-block;padding:10px'>Box</span> <span id=t>after</span></div>",
    );
    let block = find(&document, &display, "b");
    let text = find(&document, &display, "t");
    assert!(block.width < 100.0);
    assert!(text.lines[0].x > block.x + block.width);
    assert!(text.lines[0].y > block.y && text.lines[0].y < block.y + block.height);
}

/** Text next to an inline-block shares its baseline: both runs start at the same y. */
#[test]
fn inline_block_baseline_aligns_with_text() {
    let (document, display) = layout(
        "<div><span id=b style='display:inline-block;padding-top:20px'>Box</span><span id=t>after</span></div>",
    );
    let block = find(&document, &display, "b");
    let text = find(&document, &display, "t");
    assert!(close(block.lines[0].y, text.lines[0].y));
}

/** vertical-align: top pins an inline-block to the line top even beside taller content. */
#[test]
fn vertical_align_top_pins_to_line_top() {
    let (document, display) = layout(
        "<div><span id=tall style='display:inline-block;height:80px'>x</span>\
         <span id=b style='display:inline-block;vertical-align:top'>y</span></div>",
    );
    let tall = find(&document, &display, "tall");
    assert!(close(find(&document, &display, "b").y, tall.y));
}

/** vertical-align: middle centers a box near the line's middle, below the top. */
#[test]
fn vertical_align_middle_centers() {
    let (document, display) = layout(
        "<div><span id=tall style='display:inline-block;height:80px'></span>\
         <span id=b style='display:inline-block;vertical-align:middle;height:20px'></span></div>",
    );
    let b = find(&document, &display, "b");
    let tall = find(&document, &display, "tall");
    assert!(b.y > tall.y + 20.0 && b.y < tall.y + tall.height);
}

/** Inline padding and borders take room on the line and enlarge the painted box. */
#[test]
fn inline_padding_and_border_take_room() {
    let (document, display) = layout(
        "<p id=p>one<a id=l href=x style='padding:0 10px;border:1px solid red'>two</a>three</p>",
    );
    let first = &find(&document, &display, "p").lines[0];
    let link = find(&document, &display, "l");
    assert!(close(link.lines[0].x, first.x + first.width + 11.0));
    assert!(close(link.x, first.x + first.width));
    let after = display
        .items
        .iter()
        .flat_map(|item| &item.lines)
        .find(|line| line.text == "three")
        .unwrap();
    assert!(close(after.x, link.x + link.width));
}

/** text-align: center still centers lines after baseline alignment. */
#[test]
fn text_align_center_centers_lines() {
    let (document, display) =
        layout("<div style='text-align:center;width:400px'><span id=t>hi</span></div>");
    let line = &find(&document, &display, "t").lines[0];
    assert!(close(line.x - 8.0, 400.0 - (line.x - 8.0 + line.width)));
}

/** Images take their CSS height and keep their aspect ratio from it. */
#[test]
fn image_css_height_sets_size() {
    let mut document = compute_styles(
        &parse("<img id=i src=x width=40 height=20 style='height:10px;width:auto'>").unwrap(),
        &[],
    )
    .unwrap();
    document.nodes[0].image_size = Some((40, 20));
    let display = render(&document, 800.0).unwrap();
    let image = find(&document, &display, "i");
    assert!(close(image.height, 10.0));
    assert!(close(image.width, 20.0));
}

/** Hostile sizes stay bounded instead of producing unbounded geometry. */
#[test]
fn hostile_heights_stay_bounded() {
    let (_, display) = layout(
        "<div style='height:99999999px;min-height:1e300px'>x</div><div style='float:left;width:-5px'>y</div>",
    );
    assert!(display.height.is_finite());
}
