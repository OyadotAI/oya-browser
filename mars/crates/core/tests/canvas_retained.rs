/** Retained geometry preserves independent copies and admits complete additions before mutation. */
use mars_core::canvas::{Affine, Path};

/** Overflow in a later transformed contour leaves the receiving path byte-for-byte unchanged. */
#[test]
fn transformed_addition_is_atomic_on_geometry_overflow() {
    let mut source = Path::default();
    source.rect([0.0, 0.0, 2.0, 2.0], Affine::IDENTITY).unwrap();
    let mut target = source.clone();
    let before = serde_json::to_value(&target).unwrap();
    assert!(
        target
            .add_path(&source, Affine([1.0, 0.0, 0.0, 1.0, 1.0e13, 0.0]))
            .is_err()
    );
    assert_eq!(serde_json::to_value(&target).unwrap(), before);
}

/** The trailing current-point subpath is included in admission before any source contour is appended. */
#[test]
fn addition_charges_the_trailing_current_point_atomically() {
    let mut source = Path::default();
    source.rect([0.0, 0.0, 2.0, 2.0], Affine::IDENTITY).unwrap();
    let mut target = Path::default();
    for _ in 0..255 {
        target.rect([0.0, 0.0, 1.0, 1.0], Affine::IDENTITY).unwrap();
    }
    assert_eq!(target.point_count(), 1020);
    assert!(target.add_path(&source, Affine::IDENTITY).is_err());
    assert_eq!(target.point_count(), 1020);
}

/** Copy transformation preserves source coordinates and appending resumes at the closed source's start. */
#[test]
fn copies_transform_independently_and_preserve_logical_current_point() {
    let mut source = Path::default();
    source.rect([1.0, 2.0, 2.0, 2.0], Affine::IDENTITY).unwrap();
    let original = serde_json::to_value(&source).unwrap();
    let mut target = Path::default();
    target
        .add_path(&source, Affine([1.0, 0.0, 0.0, 1.0, 4.0, 0.0]))
        .unwrap();
    target.line_to([7.0, 4.0], Affine::IDENTITY).unwrap();
    let stored = serde_json::to_value(&target).unwrap();
    assert_eq!(
        stored["subpaths"][1]["points"],
        serde_json::json!([[5.0, 2.0], [7.0, 4.0]])
    );
    assert_eq!(serde_json::to_value(&source).unwrap(), original);
}
