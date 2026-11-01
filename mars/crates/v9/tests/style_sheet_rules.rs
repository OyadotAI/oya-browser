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
