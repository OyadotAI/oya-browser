/** Resize observations use final native layout boxes and resolved padding rather than transformed paint dimensions. */
use mars_eye::{compute_styles, parse, render};

#[test]
fn content_and_border_measurements_remain_distinct_after_rotation() {
    let document = compute_styles(&parse("<div id=target>sample</div>").unwrap(), &["#target{width:100px;height:50px;padding:10px;border:5px solid;transform:rotate(45deg)}".into()]).unwrap();
    let display = render(&document, 800.0).unwrap();
    let metrics = display
        .items
        .iter()
        .find(|item| item.local_id == document.nodes[0].local_id)
        .unwrap()
        .resize_box
        .unwrap();
    assert_eq!(metrics.content, [10.0, 10.0, 100.0, 50.0]);
    assert_eq!(metrics.border, [130.0, 80.0]);
}

#[test]
fn percentage_padding_is_measured_after_resolution_against_the_container() {
    let document = compute_styles(
        &parse("<section><div id=target>sample</div></section>").unwrap(),
        &["section{width:200px}#target{width:100px;height:50px;padding:10%}".into()],
    )
    .unwrap();
    let node = document
        .nodes
        .iter()
        .find(|node| node.html_id == "target")
        .unwrap();
    let display = render(&document, 800.0).unwrap();
    let metrics = display
        .items
        .iter()
        .find(|item| item.local_id == node.local_id)
        .unwrap()
        .resize_box
        .unwrap();
    assert_eq!(metrics.content, [20.0, 20.0, 100.0, 50.0]);
    assert_eq!(metrics.border, [140.0, 90.0]);
}

#[test]
fn wrapper_metrics_resolve_percentage_edges_without_including_positioned_scroll_overflow() {
    let document = compute_styles(
        &parse("<div id=out></div>").unwrap(),
        &["html{margin:0;width:400px;padding:10%}body{margin:0;width:100px;height:50px;padding:10%}#out{position:absolute;top:2000px;width:10px;height:10px}".into()],
    ).unwrap();
    let display = render(&document, 800.0).unwrap();
    let html = display.resize_wrappers.html.unwrap();
    let body = display.resize_wrappers.body.unwrap();
    assert_eq!(html.content, [80.0, 80.0, 400.0, 130.0]);
    assert_eq!(html.border, [560.0, 290.0]);
    assert_eq!(body.content, [40.0, 40.0, 100.0, 50.0]);
    assert_eq!(body.border, [180.0, 130.0]);
    assert!(display.height > 2000.0);
}
