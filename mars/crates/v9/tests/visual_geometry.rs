/** DOM visual geometry uses native transforms while offset and client measurements keep raw layout sizes. */
use crate::common;
use mars_v9::{NodeBox, PageEnvironment};

/** One visible native paragraph with explicit raw layout data. */
fn environment(bounds: NodeBox) -> PageEnvironment {
    PageEnvironment {
        geometry: vec![bounds],
        ..common::environment()
    }
}

/** Exercise geometry through real DOM APIs and surface script failures alongside the result. */
fn value(source: &str, environment: &PageEnvironment) -> String {
    let execution = common::load_with(source, environment);
    assert_eq!(common::failure(&execution), None);
    common::global(&execution, "r")
}

#[test]
fn visual_scale_changes_client_rectangle_but_not_offset_or_client_sizes() {
    let mut environment = environment(NodeBox {
        node: 1,
        x: 20.0,
        y: 30.0,
        width: 400.0,
        height: 200.0,
        layout_position: Some([10.0, 20.0]),
        layout_size: Some([200.0, 100.0]),
        ..NodeBox::default()
    });
    environment.scroll_y = 10.0;
    assert_eq!(
        value(
            "const n=document.getElementById('first');const b=n.getBoundingClientRect();const r=[b.x,b.y,b.width,b.height,n.offsetLeft,n.offsetTop,n.offsetWidth,n.offsetHeight,n.clientWidth,n.clientHeight].join('|');",
            &environment
        ),
        "20|20|400|200|10|20|200|100|200|100"
    );
}

#[test]
fn rotated_quad_rejects_empty_bounding_box_corners_and_preserves_paint_order() {
    let mut environment = environment(NodeBox {
        node: 0,
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 100.0,
        ..NodeBox::default()
    });
    environment.geometry.push(NodeBox {
        node: 1,
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 100.0,
        quad: Some([[50.0, 0.0], [100.0, 50.0], [50.0, 100.0], [0.0, 50.0]]),
        ..NodeBox::default()
    });
    assert_eq!(
        value(
            "const r=[document.elementFromPoint(5,5).id,document.elementFromPoint(50,50).id,document.elementsFromPoint(50,50).map(n=>n.id||n.tagName).join(',')].join('|');",
            &environment
        ),
        "main|first|first,main,BODY,HTML"
    );
}

#[test]
fn clip_limits_hits_without_shrinking_the_client_rectangle() {
    let environment = environment(NodeBox {
        node: 1,
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 100.0,
        clip: Some([25.0, 25.0, 50.0, 50.0]),
        ..NodeBox::default()
    });
    assert_eq!(
        value(
            "const n=document.getElementById('first');const r=[n.getBoundingClientRect().width,document.elementFromPoint(10,10),document.elementFromPoint(50,50).id].join('|');",
            &environment
        ),
        "100||first"
    );
}

#[test]
fn pinned_visual_geometry_and_clip_follow_scroll_without_moving_layout_offsets() {
    let mut environment = environment(NodeBox {
        node: 1,
        x: 10.0,
        y: 100.0,
        width: 50.0,
        height: 40.0,
        layout_position: Some([10.0, 100.0]),
        layout_size: Some([50.0, 40.0]),
        clip: Some([10.0, 100.0, 50.0, 40.0]),
        pin: Some([0.0, 100.0, 500.0]),
        ..NodeBox::default()
    });
    environment.scroll_y = 200.0;
    assert_eq!(
        value(
            "const n=document.getElementById('first');const r=[n.getBoundingClientRect().y,n.offsetTop,document.elementFromPoint(20,10).id].join('|');",
            &environment
        ),
        "0|100|first"
    );
}

#[test]
fn degenerate_or_nonfinite_quads_cannot_claim_a_hit() {
    for quad in [[[0.0; 2]; 4], [[f64::NAN; 2]; 4]] {
        let environment = environment(NodeBox {
            node: 1,
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
            quad: Some(quad),
            ..NodeBox::default()
        });
        assert_eq!(
            value(
                "const r=String(document.elementFromPoint(50,50));",
                &environment
            ),
            "null"
        );
    }
}

/** Excluding a pointer target must not discard its dimensions or hide the box underneath. */
#[test]
fn pointer_excluded_boxes_keep_measurements_but_never_appear_in_hits() {
    let mut environment = environment(NodeBox {
        node: 0,
        width: 200.0,
        height: 100.0,
        ..NodeBox::default()
    });
    environment.geometry.push(NodeBox {
        node: 1,
        width: 200.0,
        height: 100.0,
        pointer_excluded: true,
        ..NodeBox::default()
    });
    assert_eq!(
        value(
            "const n=document.getElementById('first');const r=[n.getBoundingClientRect().width,n.offsetHeight,document.elementFromPoint(50,50).id,document.elementsFromPoint(50,50).map(n=>n.id||n.tagName).join(',')].join('|');",
            &environment
        ),
        "200|100|main|main,BODY,HTML"
    );
}
