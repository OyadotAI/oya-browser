/**
 * The 4x4 matrix algebra behind CSS 3D transforms: composition, the affine and planar lifts, the
 * perspective matrix, and the projection of a box's plane to the screen. Matrices are stored in
 * `matrix3d()` order (column-major), so a matrix as written in CSS reads straight into an array.
 * Paint stays 2D: a planar item is painted through the 2D homography these reduce to, and a
 * box's facing comes from its projected corners, so both follow from one algebra.
 */
/** A 4x4 matrix in `matrix3d()` order: element (row, column) lives at `column * 4 + row`. */
pub type Matrix4 = [f64; 16];

/** The identity transform. */
pub const IDENTITY4: Matrix4 = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];

/** The index of the perspective term `-1/d` in `perspective(d)`: row 4, column 3. */
const PERSPECTIVE_INDEX: usize = 11;
/** How far a z-term may stray from its planar value and still count as planar: rotations about z
leave `1 - cos + cos` a unit short of the last bit, which must not make a 2D transform 3D. */
const PLANAR_EPSILON: f64 = 1e-12;
/** Below this w a projected point sits on or behind the eye, where no screen position exists. */
const EYE_PLANE: f64 = 1e-12;

/** `outer` applied after `inner`: the product `outer x inner` of column vectors. */
pub fn multiply(outer: &Matrix4, inner: &Matrix4) -> Matrix4 {
    std::array::from_fn(|index| {
        let (column, row) = (index / 4, index % 4);
        (0..4)
            .map(|k| outer[k * 4 + row] * inner[column * 4 + k])
            .sum()
    })
}

/** An affine `matrix(a, b, c, d, e, f)` lifted to 4x4: its z row and column are the identity's. */
pub fn from_affine([a, b, c, d, e, f]: [f64; 6]) -> Matrix4 {
    [
        a, b, 0.0, 0.0, c, d, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, e, f, 0.0, 1.0,
    ]
}

/** The 2D `matrix()` a planar 4x4 matrix is: its x and y columns, z dropped. */
pub fn planar(m: &Matrix4) -> [f64; 6] {
    [m[0], m[1], m[4], m[5], m[12], m[13]]
}

/** Whether a matrix leaves the z axis alone, so it paints exactly as its planar form does. */
pub fn is_planar(m: &Matrix4) -> bool {
    [2, 3, 6, 7, 8, 9, 11, 14]
        .iter()
        .all(|&i| m[i].abs() <= PLANAR_EPSILON)
        && (m[10] - 1.0).abs() <= PLANAR_EPSILON
        && (m[15] - 1.0).abs() <= PLANAR_EPSILON
}

/** `perspective(d)`: the matrix that divides x and y by `1 - z / d` (CSS Transforms 2). */
pub fn perspective(distance: f64) -> Matrix4 {
    let mut m = IDENTITY4;
    m[PERSPECTIVE_INDEX] = -1.0 / distance;
    m
}

/** Drops the z output of a matrix, so everything under a flat element lands in its plane. */
pub fn flatten(m: &Matrix4) -> Matrix4 {
    let mut out = *m;
    for row_z in [2, 6, 10, 14] {
        out[row_z] = 0.0;
    }
    out
}

/** A translation by `[x, y, z]`. */
pub fn translation([x, y, z]: [f64; 3]) -> Matrix4 {
    let mut m = IDENTITY4;
    m[12] = x;
    m[13] = y;
    m[14] = z;
    m
}

/** Projects a point in the plane z = 0 through `m` to screen coordinates; None when the point
lies on or behind the eye plane, where Chrome clips and Eye treats the box as absent. */
pub fn project(m: &Matrix4, [x, y]: [f64; 2]) -> Option<[f64; 2]> {
    let w = m[3] * x + m[7] * y + m[15];
    if !w.is_finite() || w <= EYE_PLANE {
        return None;
    }
    let sx = (m[0] * x + m[4] * y + m[12]) / w;
    let sy = (m[1] * x + m[5] * y + m[13]) / w;
    (sx.is_finite() && sy.is_finite()).then_some([sx, sy])
}

/** Signed area of the projected triangle `a b c`: positive when the three turn counter-clockwise
on screen as a box's top-left, top-right and bottom-left corners do when it faces the viewer. */
pub fn orientation(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

/** `num / den`, or zero for an empty box, which has no edge direction to map. */
fn safe_div(num: f64, den: f64) -> f64 {
    if den > 0.0 { num / den } else { 0.0 }
}

/** The affine map sending `(x0, y0)`, `(x0 + w, y0)` and `(x0, y0 + h)` to the given points,
which is the projection exactly at those three corners. Paint carries this affine, and the
fourth corner's residual is the perspective error a planar affine cannot show. */
pub fn fit_affine([x0, y0, w, h]: [f64; 4], [p0, p1, p2]: [[f64; 2]; 3]) -> [f64; 6] {
    let (a, b) = (safe_div(p1[0] - p0[0], w), safe_div(p1[1] - p0[1], w));
    let (c, d) = (safe_div(p2[0] - p0[0], h), safe_div(p2[1] - p0[1], h));
    [a, b, c, d, p0[0] - a * x0 - c * y0, p0[1] - b * x0 - d * y0]
}

#[cfg(test)]
mod tests {
    use super::*;

    /** Composition applies the inner matrix first: a translation after a scale moves unscaled. */
    #[test]
    fn outer_applies_after_inner() {
        let scale = from_affine([2.0, 0.0, 0.0, 2.0, 0.0, 0.0]);
        let shift = translation([3.0, 0.0, 0.0]);
        assert_eq!(
            project(&multiply(&shift, &scale), [1.0, 0.0]),
            Some([5.0, 0.0])
        );
        assert_eq!(
            project(&multiply(&scale, &shift), [1.0, 0.0]),
            Some([8.0, 0.0])
        );
    }

    /** `perspective(d)` puts -1/d in the w row of the z column, so depth shrinks points toward the origin. */
    #[test]
    fn perspective_divides_by_depth() {
        let p = perspective(100.0);
        let pushed = multiply(&p, &translation([0.0, 0.0, 50.0]));
        // w = 1 - 50/100 = 0.5, so x = 10 becomes 20.
        assert_eq!(project(&pushed, [10.0, 0.0]), Some([20.0, 0.0]));
    }

    /** A point at or behind the eye has no screen position. */
    #[test]
    fn points_behind_the_eye_do_not_project() {
        let p = multiply(&perspective(100.0), &translation([0.0, 0.0, 200.0]));
        assert_eq!(project(&p, [1.0, 1.0]), None);
    }

    /** Flattening keeps the planar part and removes any z the subtree produced. */
    #[test]
    fn flatten_drops_the_z_output() {
        let rotated = multiply(&translation([0.0, 0.0, 9.0]), &IDENTITY4);
        let flat = flatten(&rotated);
        assert_eq!(project(&flat, [1.0, 1.0]), Some([1.0, 1.0]));
        assert_eq!(project(&flatten(&IDENTITY4), [2.0, 3.0]), Some([2.0, 3.0]));
    }

    /** The planar lift round-trips the affine it came from. */
    #[test]
    fn affine_lift_round_trips() {
        let m = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        assert_eq!(planar(&from_affine(m)), m);
        assert!(is_planar(&from_affine(m)));
        assert!(!is_planar(&perspective(10.0)));
    }

    /** A three-corner fit reproduces an affine map exactly at every corner. */
    #[test]
    fn three_corner_fit_is_exact_for_affine_maps() {
        let m = from_affine([0.5, 0.25, -0.25, 1.5, 7.0, 3.0]);
        let rect = [10.0, 20.0, 40.0, 30.0];
        let corners = [[10.0, 20.0], [50.0, 20.0], [10.0, 50.0]].map(|p| project(&m, p).unwrap());
        let fit = fit_affine(rect, corners);
        let expected = planar(&m);
        for (a, b) in fit.iter().zip(expected.iter()) {
            assert!((a - b).abs() < 1e-9, "{fit:?} != {expected:?}");
        }
    }

    /** Orientation is positive for an upright box and flips when the box is mirrored. */
    #[test]
    fn mirrored_boxes_face_away() {
        let upright = orientation([0.0, 0.0], [10.0, 0.0], [0.0, 10.0]);
        let mirrored = orientation([10.0, 0.0], [0.0, 0.0], [10.0, 10.0]);
        assert!(upright > 0.0 && mirrored < 0.0);
    }
}
