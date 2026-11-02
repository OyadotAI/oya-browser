/** CSS past the rule and sheet bounds is dropped without letting hidden content become agent-actionable. */
use mars_eye::{
    Document, compute_styles,
    constants::{CSS_RULES, STYLE_BYTES},
    parse,
};

/** The node with `id`. */
fn actionable(document: &Document, id: &str) -> bool {
    document
        .nodes
        .iter()
        .find(|node| node.html_id == id)
        .unwrap()
        .style
        .interactive
}

/** A hiding rule after CSS_RULES ordinary rules is kept as a mask over its target. */
#[test]
fn hiding_rule_past_the_rule_bound_still_masks() {
    let document = parse("<input id=h><input id=v>").unwrap();
    let sheet = ".x{color:blue}".repeat(CSS_RULES) + "#h{display:none}";
    let styled = compute_styles(&document, &[sheet]).unwrap();
    assert!(!actionable(&styled, "h"));
    assert!(actionable(&styled, "v"));
    assert!(!styled.formatting.agent_untrusted);
}

/** A hiding rule in a sheet skipped for size still masks its target. */
#[test]
fn hiding_rule_in_a_skipped_sheet_still_masks() {
    let document = parse("<input id=h><input id=v>").unwrap();
    let mut sheet = "#h{opacity:0}".to_string();
    sheet.push_str(&" ".repeat(STYLE_BYTES));
    let styled = compute_styles(&document, &[sheet]).unwrap();
    assert!(!actionable(&styled, "h"));
    assert!(actionable(&styled, "v"));
}

/** When even the mask reserve overflows, nothing on the page is agent-actionable. */
#[test]
fn mask_reserve_overflow_fails_closed() {
    let document = parse("<input id=v>").unwrap();
    let hiding = ".h{display:none}".repeat(CSS_RULES);
    let ordinary = ".x{color:blue}".repeat(CSS_RULES);
    let styled = compute_styles(&document, &[ordinary, hiding.clone(), hiding]).unwrap();
    assert!(styled.formatting.agent_untrusted);
    assert!(!actionable(&styled, "v"));
    assert!(!styled.formatting.body.style.interactive);
}
