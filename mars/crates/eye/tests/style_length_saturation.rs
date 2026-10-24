/**
 * Huge finite lengths are valid and saturate, as Blink's float lengths do at layout: Tailwind 4's
 * `rounded-full` is `calc(infinity * 1px)`, serialized `3.40282e38px`, and must still round the
 * corners. Values a property bounds are then dropped by that property alone.
 */
use mars_eye::{Document, compute_styles, parse};

/** `#t` styled with `declarations`. */
fn styled(declarations: &str) -> Document {
    let sheet = format!("#t {{ width: 100px; {declarations} }}");
    compute_styles(&parse("<p id=t>x</p>").unwrap(), &[sheet]).unwrap()
}

#[test]
fn a_huge_border_radius_rounds_fully() {
    let document = styled("border-radius: 3.40282e38px");
    let node = document.nodes.iter().find(|n| n.html_id == "t").unwrap();
    assert!(node.style.radius.iter().all(|r| *r >= 50.0));
}

#[test]
fn a_huge_margin_is_still_dropped_without_losing_the_element_styles() {
    let document = styled("margin-left: 3.40282e38px; color: red");
    let node = document.nodes.iter().find(|n| n.html_id == "t").unwrap();
    assert_eq!(node.style.color.as_deref(), Some("red"));
    assert_eq!(node.style.width, Some(100.0));
}
