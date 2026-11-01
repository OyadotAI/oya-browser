/** Agent masks from rules Eye cannot parse stay as narrow as the rule's real subject, so link
clicks keep working on MDN, Google and X (each repro is cut down from that site's CSS). */
use mars_eye::{Document, compute_styles, constants::STYLE_BYTES, parse};

/** Styles `markup` with one sheet. */
fn styled(markup: &str, sheet: &str) -> Document {
    compute_styles(&parse(markup).unwrap(), &[sheet.to_owned()]).unwrap()
}
/** Whether the element with this id is agent-actionable. */
fn actionable(document: &Document, id: &str) -> bool {
    document
        .nodes
        .iter()
        .find(|node| node.html_id == id)
        .unwrap_or_else(|| panic!("no #{id}"))
        .style
        .interactive
}

/** MDN: a rule that applies only under `:focus-within` never masks, even through `:is()`. */
#[test]
fn dynamic_state_rule_never_masks() {
    let document = styled(
        "<nav class=a11y-menu><a id=skip href=#>Skip</a></nav><a id=link href=#>Docs</a>",
        ":is(.a11y-menu a):focus-within{top:0}:is(.a11y-menu a):focus-within{top:10px}",
    );
    assert!(actionable(&document, "skip"));
    assert!(actionable(&document, "link"));
}

/** Google: a rule whose subject is a pseudo-element never masks its originating element. */
#[test]
fn pseudo_element_rule_never_masks() {
    let document = styled(
        "<a id=link href=#>Images</a>",
        "html:active-view-transition-type(aimc)::view-transition-new(ubi):only-child{animation:x 1s}\
         html:active-view-transition-type(aimc)::view-transition-old(ubi){opacity:0}",
    );
    assert!(actionable(&document, "link"));
    assert!(document.formatting.html.style.interactive);
}

/** X: a margin or animation declaration never masks, so a `:where()` spacing utility Eye cannot
parse leaves every element actionable. */
#[test]
fn margin_and_animation_never_mask() {
    let document = styled(
        "<div class=-space-x-3><a id=one href=#>1</a><a id=two href=#>2</a></div>",
        ":where(.-space-x-3>:not(:last-child)){margin-inline-start:calc(-.75rem * calc(1 - var(--r)))}\
         .x:dir(ltr){animation:fade 1s}",
    );
    assert!(actionable(&document, "one"));
    assert!(actionable(&document, "two"));
}

/** `:is()`/`:where()` subjects come from their arguments: an unparseable hiding rule masks only
what its arguments could match. */
#[test]
fn is_subject_masks_only_its_arguments() {
    let document = styled(
        "<div class=menu><button id=inside type=button>in</button></div><a id=outside href=#>out</a>",
        ":is(.menu button:state(open)){display:none}",
    );
    assert!(!actionable(&document, "inside"));
    assert!(actionable(&document, "outside"));
}

/** Airbnb: a rule whose selector has a compound starting with a digit (`.a 744`) is invalid, so
Blink drops it silently; it must not mask the page, and its `position` cannot hide anything. */
#[test]
fn a_selector_starting_with_a_digit_is_invalid_and_masks_nothing() {
    let document = styled(
        "<a id=link href=#>Where</a><input id=where>",
        ".atm_mk_stnw88 744{position:absolute}",
    );
    assert!(actionable(&document, "link"));
    assert!(actionable(&document, "where"));
    assert!(document.formatting.body.style.interactive);
    assert!(!document.formatting.agent_untrusted);
}

/** Airbnb ships about 1.8 MB of CSS, and a sheet past the byte allowance keeps only its hiding
rules: plain declarations there never make a link or an input unclickable, and the page stays
trusted, however many such sheets it links. */
#[test]
fn links_and_inputs_stay_actionable_past_the_style_byte_allowance() {
    let big = ".x{color:blue}".repeat(STYLE_BYTES / ".x{color:blue}".len() + 1);
    let document = compute_styles(
        &parse("<a id=link href=#>Where</a><input id=where>").unwrap(),
        &[big.clone(), big],
    )
    .unwrap();
    assert!(actionable(&document, "link"));
    assert!(actionable(&document, "where"));
    assert!(!document.formatting.agent_untrusted);
}
