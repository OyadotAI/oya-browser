/** CSS 3D transforms (CSS Transforms 2): every transform function and unit, the individual
properties, perspective, preserve-3d flattening, backface visibility and transform-origin depth,
checked against hand-computed matrices and projected quads. */
use mars_eye::{DisplayItem, DisplayList, Document, compute_styles, parse, render};

/** Tolerance for values computed through trigonometry and unit conversion. */
const EPS: f64 = 1e-6;

/** A styled document and its display list at a fixed viewport, with body margin removed. */
fn layout(markup: &str) -> (Document, DisplayList) {
    let markup = format!("<body style='margin:0'>{markup}");
    let document = compute_styles(&parse(&markup).unwrap(), &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    (document, display)
}

/** The item of an element id. */
fn item<'a>(document: &Document, display: &'a DisplayList, id: &str) -> &'a DisplayItem {
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

/** The computed style of an element id. */
fn style<'a>(document: &'a Document, id: &str) -> &'a mars_eye::ComputedStyle {
    &document
        .nodes
        .iter()
        .find(|node| node.html_id == id)
        .unwrap()
        .style
}

/** Asserts two numbers agree within `EPS`. */
fn near(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < EPS, "{actual} != {expected}");
}

/** Asserts two quads agree corner by corner. */
fn near_quad(actual: [[f64; 2]; 4], expected: [[f64; 2]; 4]) {
    for (a, e) in actual.iter().zip(expected.iter()) {
        near(a[0], e[0]);
        near(a[1], e[1]);
    }
}

/** The planar `transform` of an element with the given declarations. */
fn planar(declarations: &str) -> [f64; 6] {
    let (document, _) = layout(&format!("<p id=t style='{declarations}'>x</p>"));
    style(&document, "t").transform.unwrap()
}

/** Every entry of a planar transform agrees with its expected value. */
fn near_affine(actual: [f64; 6], expected: [f64; 6]) {
    for (a, e) in actual.iter().zip(expected.iter()) {
        near(*a, *e);
    }
}

/** Two-dimensional functions reduce to their documented matrices. */
#[test]
fn two_dimensional_functions_match_their_matrices() {
    near_affine(
        planar("transform:translate(10px, 20px)"),
        [1.0, 0.0, 0.0, 1.0, 10.0, 20.0],
    );
    near_affine(
        planar("transform:translateX(7px)"),
        [1.0, 0.0, 0.0, 1.0, 7.0, 0.0],
    );
    near_affine(
        planar("transform:translateY(-3px)"),
        [1.0, 0.0, 0.0, 1.0, 0.0, -3.0],
    );
    near_affine(
        planar("transform:scale(2, 3)"),
        [2.0, 0.0, 0.0, 3.0, 0.0, 0.0],
    );
    near_affine(
        planar("transform:scaleY(-1)"),
        [1.0, 0.0, 0.0, -1.0, 0.0, 0.0],
    );
    near_affine(
        planar("transform:skewX(45deg)"),
        [1.0, 0.0, 1.0, 1.0, 0.0, 0.0],
    );
    near_affine(
        planar("transform:skewY(45deg)"),
        [1.0, 1.0, 0.0, 1.0, 0.0, 0.0],
    );
    near_affine(
        planar("transform:rotate(90deg)"),
        [0.0, 1.0, -1.0, 0.0, 0.0, 0.0],
    );
    near_affine(
        planar("transform:matrix(1, 2, 3, 4, 5, 6)"),
        [1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
    );
}

/** Every length unit and angle unit, and `calc()`, resolve to the same matrix as plain px and deg. */
#[test]
fn units_and_calc_resolve_like_their_plain_values() {
    near_affine(
        planar("transform:translate(1in, 0)"),
        [1.0, 0.0, 0.0, 1.0, 96.0, 0.0],
    );
    near_affine(
        planar("transform:translate(2.54cm, 0)"),
        [1.0, 0.0, 0.0, 1.0, 96.0, 0.0],
    );
    near_affine(
        planar("transform:translate(10mm, 0)"),
        [1.0, 0.0, 0.0, 1.0, 37.795_275_59, 0.0],
    );
    near_affine(
        planar("transform:translate(1rem, 2em)"),
        [1.0, 0.0, 0.0, 1.0, 16.0, 32.0],
    );
    near_affine(
        planar("transform:translate(calc(10px + 5px), 0)"),
        [1.0, 0.0, 0.0, 1.0, 15.0, 0.0],
    );
    near_affine(
        planar("transform:translate(calc(-2 * 4px), 0)"),
        [1.0, 0.0, 0.0, 1.0, -8.0, 0.0],
    );
    near_affine(
        planar("transform:rotate(0.25turn)"),
        [0.0, 1.0, -1.0, 0.0, 0.0, 0.0],
    );
    near_affine(
        planar("transform:rotate(100grad)"),
        [0.0, 1.0, -1.0, 0.0, 0.0, 0.0],
    );
    near_affine(
        planar("transform:rotate(1.5707963267948966rad)"),
        [0.0, 1.0, -1.0, 0.0, 0.0, 0.0],
    );
    near_affine(
        planar("transform:scale(calc(26 / 28))"),
        [26.0 / 28.0, 0.0, 0.0, 26.0 / 28.0, 0.0, 0.0],
    );
}

/** A translation with percentages resolves against the box, and a rotation before it carries
the translation through the rotation: `rotate(90deg) translate(10%, 0)` moves a 100px box down by
10px, not left. */
#[test]
fn percentage_translate_after_a_rotation_is_carried_through_it() {
    let (document, display) = layout(
        "<p id=t style='width:100px;height:50px;transform:rotate(90deg) translate(10%, 0)'>x</p>",
    );
    let transform = item(&document, &display, "t").transform.unwrap();
    // The centre (50, 25) maps to (50, 35): rotated 90 degrees about itself, then 10px down.
    near_affine(transform, [0.0, 1.0, -1.0, 0.0, 75.0, -15.0]);
}

/** Three-dimensional functions keep their z terms; a planar result keeps no 4x4 matrix. */
#[test]
fn three_dimensional_functions_build_their_matrices() {
    // translateZ(0) is planar; a real z offset is spatial, since perspective would scale it.
    let (document, _) = layout("<p id=a style='transform:translateZ(0)'>a</p>");
    assert!(style(&document, "a").transform_4.is_none());
    let (document, _) = layout("<p id=z style='transform:translateZ(10px)'>z</p>");
    assert_eq!(style(&document, "z").transform_4.unwrap()[14], 10.0);
    let (document, _) = layout("<p id=b style='transform:rotateY(60deg)'>b</p>");
    let style_b = style(&document, "b");
    near_affine(style_b.transform.unwrap(), [0.5, 0.0, 0.0, 1.0, 0.0, 0.0]);
    let m = style_b.transform_4.unwrap();
    near(m[0], 0.5);
    near(m[2], -(3.0_f64.sqrt() / 2.0));
    near(m[8], 3.0_f64.sqrt() / 2.0);
    let (document, _) = layout("<p id=c style='transform:translate3d(1px, 2px, 3px)'>c</p>");
    let m = style(&document, "c").transform_4.unwrap();
    near(m[12], 1.0);
    near(m[13], 2.0);
    near(m[14], 3.0);
}

/** `rotate3d()` normalises its axis, a zero axis is the identity, and `scale3d()` scales z. */
#[test]
fn rotate3d_scale3d_and_perspective_functions() {
    near_affine(
        planar("transform:rotate3d(0, 0, 2, 90deg)"),
        [0.0, 1.0, -1.0, 0.0, 0.0, 0.0],
    );
    near_affine(
        planar("transform:rotate3d(0, 0, 0, 90deg)"),
        [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
    );
    let (document, _) = layout("<p id=s style='transform:scale3d(2, 3, 4)'>s</p>");
    let m = style(&document, "s").transform_4.unwrap();
    near(m[0], 2.0);
    near(m[5], 3.0);
    near(m[10], 4.0);
    let (document, _) = layout("<p id=p style='transform:perspective(100px)'>p</p>");
    near(style(&document, "p").transform_4.unwrap()[11], -0.01);
}

/** A matrix3d() keeps its sixteen entries in CSS order. */
#[test]
fn matrix3d_keeps_its_entries_in_css_order() {
    let (document, _) =
        layout("<p id=m style='transform:matrix3d(1,0,0,0, 0,1,0,0, 0,0,2,0, 5,6,7,1)'>m</p>");
    let m = style(&document, "m").transform_4.unwrap();
    assert_eq!(m[10], 2.0);
    assert_eq!((m[12], m[13], m[14]), (5.0, 6.0, 7.0));
}

/** Percentages have no z: `translate3d` and `translateZ` refuse them, as the spec does. */
#[test]
fn z_arguments_refuse_percentages() {
    let (document, _) = layout("<p id=a style='transform:translateZ(10%)'>a</p>");
    assert_eq!(style(&document, "a").transform, None);
    let (document, _) = layout("<p id=b style='transform:translate3d(1px, 1px, 5%)'>b</p>");
    assert_eq!(style(&document, "b").transform, None);
}

/** A perspective on a parent projects its child's z-translated box about the perspective origin:
with `perspective: 100px` and the child lifted 50px toward the viewer, every point moves outward
from the parent's centre by 1 / (1 - 50/100) = 2. */
#[test]
fn perspective_magnifies_a_lifted_child_about_the_perspective_origin() {
    let (document, display) = layout(
        "<div id=p style='perspective:100px;width:200px;height:100px'>\
         <p id=c style='transform:translateZ(50px);width:40px;height:20px;margin:0'>c</p></div>",
    );
    let child = item(&document, &display, "c");
    let quad = child
        .quad
        .expect("a box in a 3D context carries its projected quad");
    // Corners (0,0), (40,0), (40,20), (0,20) about the parent centre (100, 50), doubled outward.
    near_quad(
        quad,
        [
            [-100.0, -50.0],
            [-20.0, -50.0],
            [-20.0, -10.0],
            [-100.0, -10.0],
        ],
    );
    // A uniform scale is affine, so the paint affine is exact.
    near_affine(
        child.transform.unwrap(),
        [2.0, 0.0, 0.0, 2.0, -100.0, -50.0],
    );
}

/** `perspective-origin` moves the vanishing point: with it at the parent's top-left corner, the
same lift shrinks distances from that corner instead of the centre. */
#[test]
fn perspective_origin_is_the_vanishing_point() {
    let (document, display) = layout(
        "<div id=p style='perspective:100px;perspective-origin:0 0;width:200px;height:100px'>\
         <p id=c style='transform:translateZ(50px);width:40px;height:20px;margin:0'>c</p></div>",
    );
    let quad = item(&document, &display, "c").quad.unwrap();
    // Corner (40, 20) is 2x away from the origin at (0, 0): (80, 40) lifted to w = 0.5.
    near_quad(quad, [[0.0, 0.0], [80.0, 0.0], [80.0, 40.0], [0.0, 40.0]]);
}

/** A flat parent flattens its child's rotation into its plane, so the perspective above it does
not reach the child's depth; preserve-3d keeps the child in the grandparent's 3D context, where
its z foreshortens and the projected box widens. Both quads are computed in closed form. */
#[test]
fn flat_parents_flatten_and_preserve_3d_keeps_depth() {
    let child = "<p id=c style='transform:rotateY(60deg);width:40px;height:20px;margin:0'>c</p>";
    let (document, display) = layout(&format!(
        "<div style='perspective:100px;width:200px;height:100px'>\
         <div id=p style='transform-style:flat;width:200px;height:100px'>{child}</div></div>"
    ));
    let flat = item(&document, &display, "c").quad.unwrap();
    // Flattened: x scales by cos 60 = 0.5 about the child's centre (20, 10), with no depth.
    near_quad(flat, [[10.0, 0.0], [30.0, 0.0], [30.0, 20.0], [10.0, 20.0]]);
    let (document, display) = layout(&format!(
        "<div style='perspective:100px;width:200px;height:100px'>\
         <div id=p style='transform-style:preserve-3d;width:200px;height:100px'>{child}</div></div>"
    ));
    let kept = item(&document, &display, "c").quad.unwrap();
    // Each corner (x, y, 0) becomes (X, Y, Z) with X = 20 + (x - 20) / 2, Z = -(sqrt 3 / 2)(x - 20),
    // then projects by the perspective at (100, 50): X' = (X - 100) / (1 - Z / 100) + 100, and
    // Y' = (Y - 50) / (1 - Z / 100) + 50 with Y = y.
    let project = |x: f64, y: f64| {
        let big_x = 20.0 + (x - 20.0) * 0.5;
        let z = -(3.0_f64.sqrt() / 2.0) * (x - 20.0);
        let w = 1.0 - z / 100.0;
        [(big_x - 100.0) / w + 100.0, (y - 50.0) / w + 50.0]
    };
    near_quad(
        kept,
        [
            project(0.0, 0.0),
            project(40.0, 0.0),
            project(40.0, 20.0),
            project(0.0, 20.0),
        ],
    );
    assert!((kept[1][0] - kept[0][0]) > (flat[1][0] - flat[0][0]) * 2.0);
}

/** `backface-visibility: hidden` hides a box whose front faces away: a half turn about y, or a
mirror. A quarter turn is edge-on (zero area) and stays visible, as Chrome paints it. */
#[test]
fn backface_hidden_boxes_hide_when_their_back_faces_the_viewer() {
    let hidden_when = |transform: &str| {
        let (document, display) = layout(&format!(
            "<p id=b style='backface-visibility:hidden;width:40px;height:20px;margin:0;transform:{transform}'>b</p>"
        ));
        item(&document, &display, "b")
            .clip
            .is_some_and(|clip| clip.width == 0.0)
    };
    assert!(hidden_when("rotateY(180deg)"));
    assert!(hidden_when("rotateY(120deg)"));
    assert!(hidden_when("scaleZ(-1)"));
    assert!(hidden_when("rotateX(180deg) scaleY(-1)"));
    assert!(!hidden_when("scaleX(-1)"));
    assert!(!hidden_when("rotateY(0deg)"));
    assert!(!hidden_when("rotateY(60deg)"));
    assert!(!hidden_when("none"));
}

/** Without `backface-visibility: hidden` a mirrored box stays visible, and a face turned away
inside a perspective still paints. */
#[test]
fn visible_backfaces_still_paint() {
    let (document, display) =
        layout("<p id=b style='width:40px;height:20px;margin:0;transform:rotateY(180deg)'>b</p>");
    assert!(item(&document, &display, "b").clip.is_none());
}

/** A child of a perspective parent is hidden by its own back face when the face turns away. */
#[test]
fn backface_hidden_child_under_perspective_hides() {
    let (document, display) = layout(
        "<div style='perspective:100px;width:200px;height:100px'>\
         <p id=c style='backface-visibility:hidden;transform:rotateY(180deg);width:40px;height:20px;margin:0'>c</p></div>",
    );
    assert!(
        item(&document, &display, "c")
            .clip
            .is_some_and(|clip| clip.width == 0.0)
    );
}

/** The facing of a back face is the sign of cofactor33 times the determinant (Blink's
`Transform::IsBackFaceVisible`), not the sign of the projected area: a mirror in x alone keeps the
front facing the viewer, while a mirror in z turns it away. */
#[test]
fn backface_facing_follows_the_determinant_of_the_accumulated_matrix() {
    let hidden_when = |transform: &str| {
        let (document, display) = layout(&format!(
            "<p id=b style='backface-visibility:hidden;width:40px;height:20px;margin:0;transform:{transform}'>b</p>"
        ));
        item(&document, &display, "b")
            .clip
            .is_some_and(|clip| clip.width == 0.0)
    };
    assert!(hidden_when("scaleZ(-1)"));
    assert!(hidden_when("rotateX(180deg) scaleY(-1)"));
    assert!(!hidden_when("scaleX(-1)"));
}

/** A flat parent ends the 3D context: its child's back face is judged by the child's own matrix,
so a half turn under a half turn is still turned away, not composed back to the front. */
#[test]
fn flat_parents_restart_the_backface_context() {
    let (document, display) = layout(
        "<div style='transform:rotateY(180deg);width:200px;height:100px'>\
         <p id=c style='backface-visibility:hidden;transform:rotateY(180deg);width:40px;height:20px;margin:0'>c</p></div>",
    );
    assert!(
        item(&document, &display, "c")
            .clip
            .is_some_and(|clip| clip.width == 0.0)
    );
}

/** A hidden back face under a flat parent turned away paints in the parent's plane, so its own
facing is what counts: an unturned child stays visible. */
#[test]
fn hidden_backface_child_of_a_flat_turned_parent_stays_visible() {
    let (document, display) = layout(
        "<div style='transform:rotateY(180deg);width:200px;height:100px'>\
         <p id=c style='backface-visibility:hidden;width:40px;height:20px;margin:0'>c</p></div>",
    );
    assert!(item(&document, &display, "c").clip.is_none());
}

/** A flip card's face hides its content with it: a flat box with a hidden back face paints its
children in its own plane, so they turn away and hide too. Under preserve-3d they keep their own facing. */
#[test]
fn a_hidden_back_face_hides_its_flattened_children() {
    let hidden = |style: &str| {
        let (document, display) = layout(&format!(
            "<div style='backface-visibility:hidden;width:40px;height:20px;{style}'>\
             <span id=t style='display:block;width:20px;height:10px'>t</span></div>"
        ));
        item(&document, &display, "t")
            .clip
            .is_some_and(|clip| clip.width == 0.0)
    };
    assert!(hidden("transform:rotateY(180deg)"));
    assert!(!hidden("transform:scaleX(-1)"));
    assert!(!hidden("transform:none"));
    assert!(!hidden(
        "transform:rotateY(180deg);transform-style:preserve-3d"
    ));
}

/** Individual `translate`, `rotate` and `scale` take three-dimensional forms. */
#[test]
fn individual_properties_take_three_dimensional_forms() {
    let (document, display) = layout(
        "<div style='perspective:100px;width:200px;height:100px'>\
         <p id=c style='translate:0 0 50px;width:40px;height:20px;margin:0'>c</p></div>",
    );
    let quad = item(&document, &display, "c").quad.unwrap();
    near_quad(
        quad,
        [
            [-100.0, -50.0],
            [-20.0, -50.0],
            [-20.0, -10.0],
            [-100.0, -10.0],
        ],
    );
    let (document, _) = layout("<p id=r style='rotate:x 90deg'>r</p>");
    let m = style(&document, "r").transform_4.unwrap();
    near(m[5], 0.0);
    near(m[6], 1.0);
    let (document, _) = layout("<p id=s style='scale:2 2 2'>s</p>");
    let m = style(&document, "s").transform_4.unwrap();
    near(m[10], 2.0);
}

/** `transform-origin` takes a third, depth value; a percentage there is refused. */
#[test]
fn transform_origin_takes_a_depth() {
    let (document, _) =
        layout("<p id=o style='transform-origin:10px 20px 30px;transform:rotate(1deg)'>o</p>");
    let s = style(&document, "o");
    assert_eq!(s.transform_origin, Some([10.0, 0.0, 20.0, 0.0]));
    near(s.transform_origin_z, 30.0);
    let (document, _) =
        layout("<p id=o style='transform-origin:10px 20px 30%;transform:rotate(1deg)'>o</p>");
    assert_eq!(style(&document, "o").transform_origin, None);
}

/** `perspective`, `perspective-origin`, `transform-style` and `backface-visibility` parse. */
#[test]
fn three_dimensional_properties_parse_their_keywords() {
    let (document, _) = layout(
        "<p id=a style='perspective:500px;perspective-origin:10% 20px;transform-style:preserve-3d;backface-visibility:hidden'>a</p>",
    );
    let s = style(&document, "a");
    assert_eq!(s.perspective, Some(500.0));
    assert_eq!(s.perspective_origin, Some([0.0, 10.0, 20.0, 0.0]));
    assert!(s.preserve_3d);
    assert!(s.backface_hidden);
    let (document, _) = layout("<p id=b style='perspective:none;transform-style:flat'>b</p>");
    let s = style(&document, "b");
    assert_eq!(s.perspective, None);
    assert!(!s.preserve_3d);
}

/** Invalid values for the new properties are dropped with a value-free diagnostic. */
#[test]
fn invalid_three_dimensional_values_are_dropped() {
    for declaration in [
        "perspective:-1px",
        "perspective:50%",
        "transform-style:sideways",
        "backface-visibility:maybe",
        "transform-box:nowhere",
    ] {
        let markup = format!("<p id=x style='{declaration}'>x</p>");
        let document = compute_styles(&parse(&markup).unwrap(), &[]).unwrap();
        assert!(
            document
                .diagnostics
                .iter()
                .any(|d| d.contains("not supported")),
            "{declaration} should be reported"
        );
    }
}

/** The forms Airbnb's stylesheets use parse without a diagnostic. */
#[test]
fn airbnb_forms_parse_without_a_diagnostic() {
    let markup = "<p id=x style='transform:rotateY(180deg) translateX(100%);perspective:1200px;\
        transform-style:preserve-3d;backface-visibility:hidden;transform-box:fill-box;\
        perspective-origin:0 -100%;transform:translate3d(0,100vh,0)'>x</p>";
    let document = compute_styles(&parse(markup).unwrap(), &[]).unwrap();
    assert!(
        document
            .diagnostics
            .iter()
            .all(|d| !d.contains("not supported")),
        "{:?}",
        document.diagnostics
    );
}

/** A planar box's display item stays free of projected quads, so 2D paint and hit testing are
unchanged. */
#[test]
fn planar_boxes_carry_no_projected_quad() {
    let (document, display) = layout("<p id=t style='transform:translate(4px, 5px)'>t</p>");
    assert!(item(&document, &display, "t").quad.is_none());
}

/** A deep tree under one 3D transform is placed in linear time: each box reuses its parent's
memoized context instead of recomposing every ancestor. */
#[test]
fn a_deep_tree_under_a_3d_transform_places_quickly() {
    const DEPTH: usize = 1500;
    let markup = format!(
        "<div style='transform:rotateY(10deg)'>{}x{}</div>",
        "<div>".repeat(DEPTH),
        "</div>".repeat(DEPTH)
    );
    // Layout recurses per level, so run it on the stack the engine's own service thread gets.
    let elapsed = std::thread::Builder::new()
        .stack_size(mars_eye::constants::SERVICE_STACK_BYTES)
        .spawn(move || {
            let start = std::time::Instant::now();
            let _ = layout(&markup);
            start.elapsed()
        })
        .expect("thread")
        .join()
        .expect("layout");
    // Generous: this checks the algorithm is linear, not the speed of a loaded debug CI runner.
    assert!(elapsed < std::time::Duration::from_secs(30), "{elapsed:?}");
}

/** A percentage translate after a 3D rotation carries its z too: under a 200px perspective, a 40px
box `rotateY(60deg) translateX(50%)` projects exactly as `rotateY(60deg) translateX(20px)` does. */
#[test]
fn a_percentage_translate_after_a_3d_rotation_projects_like_its_pixels() {
    let quad_of = |transform: &str| {
        let (document, display) = layout(&format!(
            "<div style='perspective:200px;width:200px;height:100px'>\
             <p id=c style='width:40px;height:20px;margin:0;transform:{transform}'>c</p></div>"
        ));
        item(&document, &display, "c").quad.unwrap()
    };
    near_quad(
        quad_of("rotateY(60deg) translateX(50%)"),
        quad_of("rotateY(60deg) translateX(20px)"),
    );
}

/** A perspective distance under 1px renders as 1px, as Blink's `UsedPerspective` does: `perspective: 0`
projects a child lifted 0.5px exactly as `perspective: 1px` does, not as an unclamped divide by zero. */
#[test]
fn a_perspective_under_one_pixel_renders_as_one_pixel() {
    let quad_of = |perspective: &str| {
        let (document, display) = layout(&format!(
            "<div style='perspective:{perspective};width:200px;height:100px'>\
             <p id=c style='transform:translateZ(0.5px);width:40px;height:20px;margin:0'>c</p></div>"
        ));
        item(&document, &display, "c").quad.unwrap()
    };
    near_quad(quad_of("0"), quad_of("1px"));
    let (document, _) = layout("<p id=p style='transform:perspective(0px)'>p</p>");
    near(style(&document, "p").transform_4.unwrap()[11], -1.0);
}

/** Each single-axis function takes exactly one argument; any other count is invalid and drops the
whole transform, as Blink's parser does. */
#[test]
fn single_axis_functions_refuse_other_argument_counts() {
    for transform in [
        "rotate(10deg 20deg)",
        "rotate(10deg, 20deg)",
        "translateX(1px, 2px)",
        "translateY(1px 2px)",
        "translateZ(1px, 2px)",
        "skewX(10deg, 5deg)",
        "skewY(1deg 2deg)",
        "rotateX(10deg 5deg)",
        "rotateY(1deg,2deg)",
        "rotateZ(1deg 2deg)",
        "scaleX(2, 3)",
        "scaleY(2 3)",
        "scaleZ(2, 3)",
        "perspective(10px, 20px)",
    ] {
        let (document, _) = layout(&format!("<p id=t style='transform:{transform}'>t</p>"));
        assert_eq!(style(&document, "t").transform, None, "{transform}");
    }
}

/** `scaleZ()` takes a percentage like the other scale functions: `scaleZ(50%)` is `scaleZ(0.5)`. */
#[test]
fn scale_z_takes_a_percentage() {
    let (document, _) = layout("<p id=s style='transform:scaleZ(50%)'>s</p>");
    near(style(&document, "s").transform_4.unwrap()[10], 0.5);
}
