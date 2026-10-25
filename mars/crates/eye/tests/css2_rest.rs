/** CSS2 normal-flow, float and margin regressions after Blink's block layout. Expected values are
Chromium's. */
use mars_eye::{compute_styles, parse, parse_xml, render};

/** The display items of `markup` laid out at 800px, with each node's id. */
fn items(markup: &str, xml: bool) -> Vec<(String, f64, f64, f64, f64)> {
    let parsed = if xml {
        parse_xml(markup, None).unwrap()
    } else {
        parse(markup).unwrap()
    };
    let sheets: Vec<String> = parsed
        .styles
        .iter()
        .filter_map(|source| match source {
            mars_eye::StyleSource::Inline(css) => Some(css.clone()),
            _ => None,
        })
        .collect();
    let document = compute_styles(&parsed, &sheets).unwrap();
    let display = render(&document, 800.0).unwrap();
    display
        .items
        .iter()
        .map(|item| {
            let id = document.nodes.iter().find(|n| n.local_id == item.local_id);
            let name = id.map_or(String::new(), |n| format!("{}#{}", item.tag, n.html_id));
            (name, item.x, item.y, item.width, item.height)
        })
        .collect()
}

/** Dumps the display items of the file at `DUMP` (a debugging aid, not a check). */
#[test]
#[ignore = "debugging aid: DUMP=<file> cargo test --test css2_rest -- --ignored dump"]
fn dump() {
    let path = std::env::var("DUMP").unwrap();
    let markup = std::fs::read_to_string(&path).unwrap();
    for (name, x, y, w, h) in items(&markup, path.ends_with(".xht")) {
        println!("{name:24} x={x:7.2} y={y:7.2} w={w:7.2} h={h:7.2}");
    }
}

/** The (y, height) of the item with html id `id`. */
fn geometry(markup: &str, id: &str) -> (f64, f64) {
    let name = format!("div#{id}");
    let (_, _, y, _, h) = items(markup, false)
        .into_iter()
        .find(|item| item.0 == name)
        .unwrap();
    (y, h)
}

/** CSS2 margin-collapse-033: a cleared self-collapsing child's margins, collapsed with a
following empty sibling's, count toward the parent's height instead of passing through it
(Blink's self_collapsing_child_had_clearance in FinishLayout). */
#[test]
fn cleared_self_collapsing_margins_stay_inside_the_parent() {
    let markup = r#"<style>body{margin:0}#c{width:100px}#f{float:left;height:1px}
        #k{clear:left}#s{margin-top:99px}</style>
        <div id="c"><div id="f"></div><div id="k"></div><div id="s"></div></div>"#;
    assert_eq!(geometry(markup, "c"), (0.0, 100.0));
}

/** CSS2 margin-collapse-clear-012: past clearance, the margins restart at the cleared child's
own top margin edge, so a following 140px margin adds only what exceeds its 40px top margin. */
#[test]
fn margins_after_clearance_restart_at_the_cleared_top_margin() {
    let markup = r#"<style>body{margin:0}#p{border-top:1px solid}#f{float:left;height:100px;width:100px}
        #k{clear:left;margin:40px 0 80px}#s{margin-bottom:140px}</style>
        <div id="p"><div id="f"></div><div id="k"></div><div id="s"></div></div>"#;
    assert_eq!(geometry(markup, "p"), (0.0, 201.0));
    assert_eq!(geometry(markup, "k").0, 101.0);
}

/** CSS2 floats-rule3-outside-left-001: a float wider than its containing block still goes on
the first line when the only float beside it lies outside that block, since an opportunity no
float narrows takes any float (Blink's FindLayoutOpportunity). */
#[test]
fn a_float_outside_the_containing_block_does_not_push_a_wide_float_down() {
    let markup = r#"<style>body{margin:0}</style><div style="float:left;width:500px;height:500px">
        <div style="float:right;width:50px;height:300px"></div>
        <div style="margin-right:100px"><div id="w" style="float:left;width:425px;height:10px"></div></div>
        </div>"#;
    assert_eq!(geometry(markup, "w"), (0.0, 10.0));
}

/** CSS2 floats-rule3-outside-left-002: a float wider than the room up to the float outside its
containing block moves below that float. */
#[test]
fn a_float_wider_than_the_room_beside_an_outside_float_moves_below_it() {
    let markup = r#"<style>body{margin:0}</style><div style="float:left;width:500px;height:500px">
        <div style="float:right;width:50px;height:300px"></div>
        <div style="margin-right:100px"><div id="w" style="float:left;width:475px;height:10px"></div></div>
        </div>"#;
    assert_eq!(geometry(markup, "w"), (300.0, 10.0));
}

/** CSS2 floats-rule7-outside-left-001: a left float beside another left float may not overhang
its containing block's right edge, even when that other float lies outside the block. */
#[test]
fn a_left_float_beside_an_outside_left_float_may_not_overhang_the_right_edge() {
    let markup = r#"<style>body{margin:0}</style><div style="float:left;width:500px;height:500px">
        <div style="float:left;width:50px;height:300px"></div>
        <div style="margin-left:100px"><div id="w" style="float:left;width:425px;height:10px"></div></div>
        </div>"#;
    assert_eq!(geometry(markup, "w"), (300.0, 10.0));
}
