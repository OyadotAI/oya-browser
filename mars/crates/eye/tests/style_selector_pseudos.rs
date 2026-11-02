/**
 * Pseudo-classes Blink's SelectorChecker evaluates from the document alone: `:lang()` (RFC 4647
 * extended filtering), `:dir()`, form states, `:open`, and `:nth-child(an+b of S)`.
 */
use mars_eye::{Document, compute_styles, parse};

/** The color of `#t` after styling `markup` with `sheet`. */
fn color(markup: &str, sheet: &str) -> Option<String> {
    let document: Document = compute_styles(&parse(markup).unwrap(), &[sheet.into()]).unwrap();
    let node = document.nodes.iter().find(|n| n.html_id == "t").unwrap();
    node.style.color.clone()
}
/** Green when `selector` matches `#t` in `markup`, else None. */
fn hit(markup: &str, selector: &str) -> bool {
    color(markup, &format!("{selector} {{ color: green }}")).as_deref() == Some("green")
}

#[test]
fn lang_matches_the_inherited_language_by_prefix_subtags() {
    let markup = "<div lang=en-US><p id=t>x</p></div>";
    assert!(hit(markup, ":lang(en)"));
    assert!(hit(markup, ":lang(EN-us)"));
    assert!(!hit(markup, ":lang(e)"));
    assert!(!hit(markup, ":lang(fr)"));
}

#[test]
fn lang_ranges_follow_extended_filtering_with_wildcards() {
    let markup = "<p id=t lang=de-Latn-DE>x</p>";
    assert!(hit(markup, ":lang(de-DE)"));
    assert!(hit(markup, ":lang(\"*-DE\")"));
    assert!(hit(markup, ":lang(fr, de)"));
    assert!(!hit(markup, ":lang(de-CH)"));
}

#[test]
fn lang_never_matches_without_a_language() {
    assert!(!hit("<p id=t>x</p>", ":lang(en)"));
}

#[test]
fn dir_follows_the_nearest_dir_attribute() {
    let markup = "<div dir=rtl><p id=t>x</p></div>";
    assert!(hit(markup, "p:dir(rtl)"));
    assert!(!hit(markup, "p:dir(ltr)"));
    assert!(hit("<p id=t>x</p>", "p:dir(ltr)"));
}

#[test]
fn placeholder_shown_matches_an_empty_field_with_a_placeholder() {
    assert!(hit(
        "<input id=t placeholder=Search>",
        "input:placeholder-shown"
    ));
    assert!(!hit(
        "<input id=t placeholder=Search value=a>",
        "input:placeholder-shown"
    ));
    assert!(hit(
        "<input id=t value=a placeholder=b>",
        "input:not(:placeholder-shown)"
    ));
}

#[test]
fn read_only_and_read_write_split_text_fields_from_everything_else() {
    assert!(hit("<input id=t>", "input:read-write"));
    assert!(hit("<input id=t readonly>", "input:read-only"));
    assert!(hit("<p id=t>x</p>", "p:read-only"));
    assert!(hit("<p id=t contenteditable>x</p>", "p:read-write"));
}

#[test]
fn required_fields_without_a_value_are_invalid() {
    assert!(hit("<input id=t required>", "input:required:invalid"));
    assert!(hit("<input id=t>", "input:optional:valid"));
    assert!(!hit("<input id=t type=submit>", "input:valid"));
}

#[test]
fn open_matches_open_details_and_dialogs() {
    assert!(hit(
        "<details id=t open><summary>s</summary></details>",
        "details:open"
    ));
    assert!(!hit(
        "<details id=t><summary>s</summary></details>",
        "details:open"
    ));
}

#[test]
fn default_matches_markup_selected_choices() {
    assert!(hit("<input id=t type=checkbox checked>", ":default"));
    assert!(!hit("<input id=t type=checkbox>", ":default"));
}

#[test]
fn nth_child_of_counts_only_siblings_matching_the_selector() {
    let markup = "<ul><li class=x>a</li><li>b</li><li class=x>c</li><li id=t class=x>d</li></ul>";
    assert!(hit(markup, "li:nth-child(3 of .x)"));
    assert!(hit(markup, "li:nth-last-child(1 of .x)"));
    assert!(!hit(markup, "li:nth-child(4 of .x)"));
}

#[test]
fn script_driven_states_parse_and_never_match() {
    let sheet = "[popover]:popover-open, dialog:modal { color: red } p { color: green }";
    assert_eq!(
        color("<p id=t popover>x</p>", sheet).as_deref(),
        Some("green")
    );
}

/** A popover is hidden until its page opens it, as Chrome's UA sheet hides
`[popover]:not(:popover-open)`; the engine-only marker V9 writes on open shows it. */
#[test]
fn popovers_are_hidden_until_opened() {
    let hidden = |markup: &str| {
        let document: Document = compute_styles(&parse(markup).unwrap(), &[]).unwrap();
        document
            .nodes
            .iter()
            .find(|n| n.html_id == "t")
            .unwrap()
            .style
            .hidden
    };
    assert!(hidden("<div id=t popover=manual>tip</div>"));
    assert!(hidden("<div id=t popover>tip</div>"));
    let mut open = parse("<div id=t popover=manual>tip</div>").unwrap();
    let node = open.nodes.iter_mut().find(|n| n.html_id == "t").unwrap();
    node.attributes
        .insert(mars_core::dom::POPOVER_OPEN_ATTRIBUTE.into(), String::new());
    let styled = compute_styles(&open, &[]).unwrap();
    assert!(
        !styled
            .nodes
            .iter()
            .find(|n| n.html_id == "t")
            .unwrap()
            .style
            .hidden
    );
    assert!(!hidden("<div id=t>plain</div>"));
}

/** Parsing keeps a page whose element has an empty or overlong id: the element gets an internal
key, where refusing the id used to refuse the whole document. */
#[test]
fn empty_and_overlong_ids_parse() {
    let long = "y".repeat(mars_core::constants::HTML_ID_BYTES + 1);
    let document = parse(&format!(
        "<div id=\"\">a</div><div id=\"{long}\">b</div><div id=t>c</div>"
    ))
    .unwrap();
    let divs: Vec<_> = document.nodes.iter().filter(|n| n.tag == "div").collect();
    assert_eq!(divs.len(), 3);
    assert!(divs[0].html_id.starts_with('@'));
    assert!(divs[1].html_id.starts_with('@'));
    assert_eq!(divs[2].html_id, "t");
}
