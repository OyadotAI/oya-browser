/** Annex B's legacy String.prototype HTML methods (CreateHTML, spec B.2.2.2.1). */
mod common;
use common::eval;

/** Each method wraps the string in its tag and quotes the attribute, escaping `"`. */
#[test]
fn html_methods_wrap_the_string_and_escape_the_attribute() {
    let source = "const out = ['x'.anchor('a\"b'), 'x'.big(), 'x'.bold(), 'x'.fixed(),
        'x'.fontcolor('red'), 'x'.fontsize(7), 'x'.italics(), 'x'.link('u'), 'x'.small(),
        'x'.strike(), 'x'.sub(), 'x'.sup(), 'x'.blink(), String.prototype.anchor.length].join('|');";
    assert_eq!(
        eval(source, "out"),
        "<a name=\"a&quot;b\">x</a>|<big>x</big>|<b>x</b>|<tt>x</tt>|<font color=\"red\">x</font>|\
         <font size=\"7\">x</font>|<i>x</i>|<a href=\"u\">x</a>|<small>x</small>|\
         <strike>x</strike>|<sub>x</sub>|<sup>x</sup>|<blink>x</blink>|1"
    );
}

/** A null or undefined receiver throws a TypeError. */
#[test]
fn html_methods_reject_a_null_receiver() {
    let source = "let out = 'none'; try { String.prototype.big.call(null); } catch (e) { out = e.constructor.name; }";
    assert_eq!(eval(source, "out"), "TypeError");
}
