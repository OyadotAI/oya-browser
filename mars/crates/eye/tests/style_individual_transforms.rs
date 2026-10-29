/** The individual transform properties, `transform-origin` and `text-shadow` compute through Eye's
public styling surface. */
use mars_eye::{Document, compute_styles, parse};

/** Styles `<p id=target>` with `declarations`, inside a parent styled with `parent`. */
fn styled(declarations: &str, parent: &str) -> Document {
    let markup =
        format!("<div style='{parent}'><p id=target style='{declarations}'>text</p></div>");
    compute_styles(&parse(&markup).unwrap(), &[]).unwrap()
}

/** The target element's computed style. */
fn target(document: &Document) -> &mars_eye::ComputedStyle {
    &document
        .nodes
        .iter()
        .find(|node| node.html_id == "target")
        .unwrap()
        .style
}

/** Rounds a matrix so trigonometry compares exactly. */
fn rounded(matrix: Option<[f64; 6]>) -> Option<[f64; 6]> {
    matrix.map(|m| m.map(|v| (v * 1e9).round() / 1e9 + 0.0))
}

/** translate, rotate and scale each produce their matrix. */
#[test]
fn individual_properties_produce_their_matrices() {
    let translate = styled("translate: 10px 20px", "");
    assert_eq!(
        target(&translate).transform,
        Some([1.0, 0.0, 0.0, 1.0, 10.0, 20.0])
    );
    let rotate = styled("rotate: 90deg", "");
    assert_eq!(
        rounded(target(&rotate).transform),
        Some([0.0, 1.0, -1.0, 0.0, 0.0, 0.0])
    );
    let scale = styled("scale: 150%", "");
    assert_eq!(
        target(&scale).transform,
        Some([1.5, 0.0, 0.0, 1.5, 0.0, 0.0])
    );
}

/** The individual properties apply before the transform list, whatever the authored order. */
#[test]
fn individual_properties_compose_ahead_of_transform() {
    let document = styled("transform: translateX(5px); scale: 2", "");
    assert_eq!(
        target(&document).transform,
        Some([2.0, 0.0, 0.0, 2.0, 10.0, 0.0])
    );
}

/** A percentage translate adds to the box-relative offset that centers overlays. */
#[test]
fn percentage_translate_is_box_relative() {
    let document = styled("translate: -50% -50%", "");
    assert_eq!(target(&document).translate_percent, [-50.0, -50.0]);
}

/** A rotation about another axis is a 3D transform Eye refuses. */
#[test]
fn rotation_about_x_is_refused() {
    let document = styled("rotate: x 45deg", "");
    assert_eq!(target(&document).transform, None);
}

/** Keywords in either order, lengths and percentages place the origin. */
#[test]
fn transform_origin_reads_keywords_lengths_and_percentages() {
    assert_eq!(
        target(&styled("transform-origin: top left", "")).transform_origin,
        Some([0.0, 0.0, 0.0, 0.0])
    );
    assert_eq!(
        target(&styled("transform-origin: 25% 10px", "")).transform_origin,
        Some([0.0, 25.0, 10.0, 0.0])
    );
    assert_eq!(
        target(&styled("transform-origin: bottom", "")).transform_origin,
        Some([0.0, 50.0, 0.0, 100.0])
    );
}

/** text-shadow inherits, and inset or a spread make the value invalid. */
#[test]
fn text_shadow_inherits_and_refuses_box_only_parts() {
    let inherited = styled("", "text-shadow: 1px 2px 3px red");
    let shadows = &target(&inherited).text_shadows;
    assert_eq!(shadows.len(), 1);
    assert_eq!(
        (shadows[0].x, shadows[0].y, shadows[0].blur),
        (1.0, 2.0, 3.0)
    );
    assert!(
        target(&styled("text-shadow: inset 1px 1px red", ""))
            .text_shadows
            .is_empty()
    );
    assert!(
        target(&styled("text-shadow: 1px 1px 0 2px red", ""))
            .text_shadows
            .is_empty()
    );
}
