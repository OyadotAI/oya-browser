/** Cascade layers as Blink orders them (CascadeLayerMap, CascadePriority) and CSS Cascade 5. */
use mars_eye::{Document, compute_styles, parse};

/** The color of `#t` after styling `<p id=t>` with `sheet`. */
fn color(sheet: &str) -> Option<String> {
    let document: Document =
        compute_styles(&parse("<p id=t class=c>x</p>").unwrap(), &[sheet.into()]).unwrap();
    let node = document.nodes.iter().find(|n| n.html_id == "t").unwrap();
    node.style.color.clone()
}

#[test]
fn unlayered_rules_beat_layered_ones_whatever_their_specificity() {
    assert_eq!(
        color("@layer base { #t.c { color: red } } p { color: green }").as_deref(),
        Some("green")
    );
}

#[test]
fn a_layer_statement_fixes_the_order_before_the_rules_appear() {
    let sheet = "@layer b, a; @layer a { p { color: green } } @layer b { #t { color: red } }";
    assert_eq!(color(sheet).as_deref(), Some("green"));
}

#[test]
fn important_declarations_invert_layer_order() {
    let sheet = "@layer a { p { color: green !important } } @layer b { p { color: red !important } }\
                 p { color: blue !important }";
    assert_eq!(color(sheet).as_deref(), Some("green"));
}

#[test]
fn sublayers_come_before_their_parent_layers_own_rules() {
    let sheet = "@layer a { p { color: green } @layer b { #t { color: red } } }";
    assert_eq!(color(sheet).as_deref(), Some("green"));
}

#[test]
fn dotted_names_reopen_the_same_sublayer() {
    let sheet = "@layer x.y { p { color: red } } @layer z { p { color: green } } @layer x { @layer y { #t { color: blue } } }";
    assert_eq!(color(sheet).as_deref(), Some("green"));
}

#[test]
fn each_anonymous_layer_is_new_and_later_ones_win() {
    let sheet = "@layer { #t { color: red } } @layer { p { color: green } }";
    assert_eq!(color(sheet).as_deref(), Some("green"));
}

#[test]
fn layers_continue_across_sheets_in_document_order() {
    let document = compute_styles(
        &parse("<p id=t>x</p>").unwrap(),
        &[
            "@layer reset, app; @layer app { p { color: green } }".into(),
            "@layer reset { #t { color: red } }".into(),
        ],
    )
    .unwrap();
    let node = document.nodes.iter().find(|n| n.html_id == "t").unwrap();
    assert_eq!(node.style.color.as_deref(), Some("green"));
}
