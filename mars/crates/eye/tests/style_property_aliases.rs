/** Legacy and prefixed property names parse as the property they alias, as in Blink. */
use mars_eye::{compute_styles, parse};

#[test]
fn grid_gap_sets_the_gap_longhands() {
    let document = compute_styles(
        &parse("<div id=t>x</div>").unwrap(),
        &["#t { display: grid; grid-gap: 12px 7px }".into()],
    )
    .unwrap();
    let node = document.nodes.iter().find(|n| n.html_id == "t").unwrap();
    assert_eq!((node.style.row_gap, node.style.column_gap), (12.0, 7.0));
}

#[test]
fn a_prefixed_alias_and_its_standard_name_cascade_as_one_property() {
    let document = compute_styles(
        &parse("<div id=t>x</div>").unwrap(),
        &[
            "#t { -webkit-box-sizing: content-box } div { box-sizing: border-box }\
           #t { -webkit-opacity: 0.5 }"
                .into(),
        ],
    )
    .unwrap();
    let node = document.nodes.iter().find(|n| n.html_id == "t").unwrap();
    assert_eq!(node.style.opacity, 0.5);
    assert_eq!(
        format!("{:?}", node.style.box_sizing),
        "ContentBox",
        "the more specific prefixed spelling wins"
    );
}
