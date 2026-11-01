/**
 * CSSStyleSheet rule deletion in V9. A style element's sheet keeps its rules as text, and
 * deleteRule removes one of them, so the rules left behind keep their order. Styled-components
 * clears its rules this way when it re-renders, and an unsupported deletion threw out of the
 * page's effects. Each test pins one rule through the public surface.
 */
use crate::common;

use common::eval;

/** deleteRule removes the rule at its index and the rules around it keep their order. */
#[test]
fn delete_rule_removes_the_rule_at_its_index() {
    let r = eval(
        "const s = document.createElement('style');
         s.textContent = 'p { color: red }\\n.a { margin: 0 }\\nb { x: 1 }';
         document.head.appendChild(s);
         const sheet = s.sheet;
         sheet.deleteRule(1);
         const r = [sheet.cssRules.length, sheet.cssRules[0].selectorText,
           sheet.cssRules[1].selectorText].join('|');",
        "r",
    );
    assert_eq!(r, "2|p|b");
}

/** An index past the last rule is an IndexSizeError, and the sheet is left as it was. */
#[test]
fn delete_rule_past_the_last_rule_throws_index_size_error() {
    let r = eval(
        "const s = document.createElement('style');
         s.textContent = 'p { color: red }';
         document.head.appendChild(s);
         let name = '';
         try { s.sheet.deleteRule(4); } catch (e) { name = e.name; }
         const r = [name, s.sheet.cssRules.length].join('|');",
        "r",
    );
    assert_eq!(r, "IndexSizeError|1");
}

/** removeRule without an index removes the first rule, as the legacy method does. */
#[test]
fn remove_rule_without_an_index_removes_the_first_rule() {
    let r = eval(
        "const s = document.createElement('style');
         s.textContent = 'p { color: red }\\nb { x: 1 }';
         document.head.appendChild(s);
         s.sheet.removeRule();
         const r = [s.sheet.cssRules.length, s.sheet.cssRules[0].selectorText].join('|');",
        "r",
    );
    assert_eq!(r, "1|b");
}

/** deleteRule keeps a nested `@media` block whole: the rules after it still follow it, rather
than nesting inside a block whose closing brace was lost. */
#[test]
fn delete_rule_keeps_a_nested_media_block_whole() {
    let r = eval(
        "const s = document.createElement('style');
         s.textContent = '@media screen and (max-width: 599px) { .m { color: red } }\\n.keep { position: fixed }\\n.gone { color: blue }\\n.after { visibility: hidden }';
         document.head.appendChild(s);
         s.sheet.deleteRule(2);
         const t = s.textContent;
         const r = [t.split('{').length === t.split('}').length, s.sheet.cssRules.length,
           s.sheet.cssRules[0].selectorText, s.sheet.cssRules[1].selectorText,
           s.sheet.cssRules[2].selectorText].join('|');",
        "r",
    );
    assert_eq!(
        r,
        "true|3|@media screen and (max-width: 599px)|.keep|.after"
    );
}

/** insertRule places the rule at the index it names, so cascade order follows the index. */
#[test]
fn insert_rule_places_the_rule_at_its_index() {
    let r = eval(
        "const s = document.createElement('style');
         s.textContent = 'p { color: red }\\nb { x: 1 }';
         document.head.appendChild(s);
         const n = s.sheet.insertRule('.ins { color: green }', 1);
         const r = [n, s.sheet.cssRules.length, s.sheet.cssRules[0].selectorText,
           s.sheet.cssRules[1].selectorText, s.sheet.cssRules[2].selectorText].join('|');",
        "r",
    );
    assert_eq!(r, "1|3|p|.ins|b");
}

/** An insertRule index past the last rule is an IndexSizeError, and the sheet is left as it was. */
#[test]
fn insert_rule_past_the_last_rule_throws_index_size_error() {
    let r = eval(
        "const s = document.createElement('style');
         s.textContent = 'p { color: red }';
         document.head.appendChild(s);
         let name = '';
         try { s.sheet.insertRule('b { x: 1 }', 5); } catch (e) { name = e.name; }
         const r = [name, s.sheet.cssRules.length].join('|');",
        "r",
    );
    assert_eq!(r, "IndexSizeError|1");
}

/** A brace inside a string does not end a rule, so the rules around it still split correctly. */
#[test]
fn braces_inside_strings_do_not_split_rules() {
    let r = eval(
        "const s = document.createElement('style');
         s.textContent = 'p { content: \"}\" }\\n.a { margin: 0 }';
         document.head.appendChild(s);
         const r = [s.sheet.cssRules.length, s.sheet.cssRules[0].selectorText,
           s.sheet.cssRules[1].selectorText].join('|');",
        "r",
    );
    assert_eq!(r, "2|p|.a");
}

/** A statement at-rule ends at its semicolon and counts as one rule of its own. */
#[test]
fn statement_at_rules_are_one_rule_each() {
    let r = eval(
        "const s = document.createElement('style');
         s.textContent = '@import url(a.css);\\np { color: red }';
         document.head.appendChild(s);
         const r = [s.sheet.cssRules.length, s.sheet.cssRules[1].selectorText].join('|');",
        "r",
    );
    assert_eq!(r, "2|p");
}
