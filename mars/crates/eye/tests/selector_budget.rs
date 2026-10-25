/** Bounds on `:nth-child(an+b of S)` matching: each argument scans every sibling of every
 * candidate, so nesting and huge sibling lists must not multiply the work. */
use mars_eye::{compute_styles, parse};

/** Whether `selector` turns `#t` green in `markup`. */
fn hit(markup: &str, selector: &str) -> bool {
    let sheet = format!("{selector} {{ color: green }}");
    let document = compute_styles(&parse(markup).unwrap(), &[sheet]).unwrap();
    let node = document.nodes.iter().find(|n| n.html_id == "t").unwrap();
    node.style.color.as_deref() == Some("green")
}

/** A nested `of S` is refused, so the rule does not apply (one level still does). */
#[test]
fn nested_nth_child_of_is_refused() {
    let markup = "<ul><li class=x id=t>a</li><li class=x>b</li></ul>";
    assert!(hit(markup, "li:nth-child(1 of .x)"));
    assert!(!hit(markup, "li:nth-child(1 of :nth-child(1 of .x))"));
    assert!(!hit(
        markup,
        "li:is(:nth-child(1 of :not(:nth-child(2 of .x))))"
    ));
}

/** Past the sibling budget the match stops instead of scanning every sibling for each one. */
#[test]
fn nth_child_of_over_a_huge_sibling_list_stops() {
    let markup = format!(
        "<ul><li class=x id=t>a</li>{}</ul>",
        "<li class=x>b</li>".repeat(2000)
    );
    assert!(!hit(&markup, "li:nth-child(1 of .x)"));
}
