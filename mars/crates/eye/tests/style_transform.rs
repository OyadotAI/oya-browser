/** CSS transform functions project supported 3D translations into Eye's affine paint data. */
use mars_eye::{Document, compute_styles, parse};

/** Compute one authored transform through the public styling surface. */
fn styled(value: &str) -> Document {
    let markup = format!("<p id=target style='transform:{value}'>text</p>");
    compute_styles(&parse(&markup).unwrap(), &[]).unwrap()
}

/** Find the authored element without depending on parser node order. */
fn matrix(document: &Document) -> Option<[f64; 6]> {
    document
        .nodes
        .iter()
        .find(|node| node.html_id == "target")
        .unwrap()
        .style
        .transform
}

#[test]
fn a_turn_rotation_and_negative_axis_scale_are_accepted() {
    let rotated = matrix(&styled("rotate(1turn)")).unwrap();
    assert!((rotated[0] - 1.0).abs() < 1e-12);
    assert!(rotated[1].abs() < 1e-12);
    assert_eq!(
        matrix(&styled("scaleX(-1)")),
        Some([-1.0, 0.0, 0.0, 1.0, 0.0, 0.0])
    );
}

#[test]
fn three_dimensional_translations_keep_their_xy_projection() {
    assert_eq!(
        matrix(&styled("translateZ(4px)")),
        Some([1.0, 0.0, 0.0, 1.0, 0.0, 0.0])
    );
    assert_eq!(
        matrix(&styled("translate3d(2px, 3px, 4px)")),
        Some([1.0, 0.0, 0.0, 1.0, 2.0, 3.0])
    );
}

#[test]
fn matrix_arguments_keep_their_authored_order() {
    assert_eq!(
        matrix(&styled("matrix(1, 2, 3, 4, 5, 6)")),
        Some([1.0, 2.0, 3.0, 4.0, 5.0, 6.0])
    );
}

#[test]
fn a_three_dimensional_translation_requires_a_length_for_z() {
    assert_eq!(matrix(&styled("translate3d(2px, 3px, 4%)")), None);
    assert_eq!(matrix(&styled("translate3d(2px, 3px)")), None);
}
