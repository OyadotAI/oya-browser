/** Structural selectors and UA hiding: of-type, bounded `:has()`, closed details and dialogs, `inert`. */
use mars_eye::{Document, compute_styles, parse};

/** Styles `markup` with `sheet`. */
fn styled(markup: &str, sheet: &str) -> Document {
    compute_styles(&parse(markup).unwrap(), &[sheet.to_string()]).unwrap()
}
/** The style of the node with `id`. */
fn style<'a>(document: &'a Document, id: &str) -> &'a mars_eye::ComputedStyle {
    &document
        .nodes
        .iter()
        .find(|node| node.html_id == id)
        .unwrap()
        .style
}

/** `:first-of-type`, `:last-of-type`, `:nth-of-type` and `:only-of-type` count same-tag siblings only. */
#[test]
fn of_type_counts_same_tag_siblings() {
    let document = styled(
        "<div><span id=s1>a</span><b id=b1>b</b><span id=s2>c</span><span id=s3>d</span><i id=i1>e</i></div>",
        "span:first-of-type{display:none}span:last-of-type{opacity:0.5}\
         span:nth-of-type(2){font-weight:bold}i:only-of-type{display:none}",
    );
    assert!(style(&document, "s1").hidden);
    assert!(!style(&document, "s2").hidden && style(&document, "s2").bold);
    assert_eq!(style(&document, "s3").opacity, 0.5);
    assert!(!style(&document, "b1").hidden);
    assert!(style(&document, "i1").hidden);
}

/** `:has()` matches by descendant, child and following sibling. */
#[test]
fn has_matches_relative_elements() {
    let document = styled(
        "<div id=a><p><em class=x>x</em></p></div><div id=b><p>y</p></div>\
         <ul id=c><li class=x>z</li></ul><h2 id=d>t</h2><p class=lede>u</p>",
        "div:has(.x){display:none}ul:has(> .x){display:none}h2:has(+ .lede){display:none}",
    );
    assert!(style(&document, "a").hidden);
    assert!(!style(&document, "b").hidden);
    assert!(style(&document, "c").hidden);
    assert!(style(&document, "d").hidden);
}

/** A hiding rule whose `:has()` runs past its budget masks its subject instead of failing open. */
#[test]
fn exhausted_has_on_a_hiding_rule_masks_its_subject() {
    let filler = "<i></i>".repeat(2000);
    let document = styled(
        &format!(
            "<div class=menu id=m>{filler}<a class=x href=/>go</a></div><div id=other>o</div>"
        ),
        ".menu:has(.x){display:none}",
    );
    assert!(!style(&document, "m").interactive);
    assert!(style(&document, "other").interactive);
}

/** An exhausted `:has()` on a rule that cannot hide is simply no match. */
#[test]
fn exhausted_has_on_a_plain_rule_is_no_match() {
    let filler = "<i></i>".repeat(2000);
    let document = styled(
        &format!("<div class=menu id=m>{filler}<b class=x>x</b></div>"),
        ".menu:has(.x){color:red}",
    );
    assert!(style(&document, "m").interactive);
    assert_eq!(style(&document, "m").color, None);
}

/** `:has()` inside `:has()` is invalid however the name is spelled, so the rule is dropped. */
#[test]
fn nested_has_is_refused_in_any_spelling() {
    for sheet in [
        "div:has(p:has(em)){color:red}",
        "div:has(p:HAS(em)){color:red}",
        "div:has(p:h\\61s(em)){color:red}",
    ] {
        let document = styled("<div id=a><p><em>x</em></p></div>", sheet);
        assert_eq!(style(&document, "a").color, None, "{sheet}");
    }
    let upper = styled("<div id=a><em>x</em></div>", "div:HAS(em){display:none}");
    assert!(style(&upper, "a").hidden);
    let escaped = styled("<div id=a><em>x</em></div>", "div:h\\61s(em){display:none}");
    assert!(style(&escaped, "a").hidden);
}

/** A closed `details` shows only its first `summary`; an open one shows everything. */
#[test]
fn closed_details_hides_all_but_its_summary() {
    let document = styled(
        "<details><summary id=s>more</summary><a id=x href=/>x</a></details>\
         <details open><summary>s</summary><a id=y href=/>y</a></details>",
        "a{display:block}",
    );
    assert!(!style(&document, "s").hidden);
    assert!(style(&document, "x").hidden && !style(&document, "x").interactive);
    assert!(!style(&document, "y").hidden);
}

/** A `dialog` without `open` is hidden even when the page gives it a display. */
#[test]
fn closed_dialog_is_hidden() {
    let document = styled(
        "<dialog id=d><button id=b>ok</button></dialog><dialog id=o open>hi</dialog>",
        "dialog{display:block}",
    );
    assert!(style(&document, "d").hidden && !style(&document, "b").interactive);
    assert!(!style(&document, "o").hidden);
}

/** `inert` content stays visible but no descendant is actionable, `pointer-events` included. */
#[test]
fn inert_subtree_is_never_actionable() {
    let document = styled(
        "<div inert id=i><button id=b>x</button></div><button id=ok>y</button>",
        "button{pointer-events:auto}",
    );
    assert!(!style(&document, "i").hidden);
    assert!(!style(&document, "b").interactive);
    assert!(style(&document, "ok").interactive);
}
