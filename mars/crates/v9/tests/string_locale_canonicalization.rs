/** String locale case methods canonicalize their locales argument, while the plain case methods ignore it. */
mod common;

/** Greek final sigma follows cased context across case-ignorable characters. */
#[test]
fn lower_case_applies_contextual_final_sigma() {
    let source = "const r=['ΚΟΣΜΟΣ ΚΟΣΜΟΣ'.toLowerCase(),'Σ'.toLowerCase(),'ΟΣ\\u0301Α'.toLowerCase(),'ΟΣ\\u0301 '.toLowerCase(),'AΣ\\ud800'.toLowerCase(),'AΣ\\u0345'.toLowerCase(),'\\u0345Σ'.toLowerCase()].join('|');";
    assert_eq!(
        common::eval(source, "r"),
        "κοσμος κοσμος|σ|οσ́α|ος́ |aς�|aςͅ|ͅσ"
    );
}

/** Public results are the observable boundary, so each check reports a string the tests compare. */
fn eval(source: &str) -> String {
    common::eval(source, "r")
}

/** An invalid language tag in the locales list is a TypeError for toLocaleLowerCase. */
#[test]
fn locale_lower_case_rejects_invalid_language_tag() {
    assert_eq!(
        eval(
            "let r;try{'I'.toLocaleLowerCase([42]);r='no throw'}catch(e){r=e instanceof TypeError?'TypeError':'other'}"
        ),
        "TypeError"
    );
}

/** An invalid language tag is a TypeError for toLocaleUpperCase too. */
#[test]
fn locale_upper_case_rejects_invalid_language_tag() {
    assert_eq!(
        eval(
            "let r;try{'i'.toLocaleUpperCase([42]);r='no throw'}catch(e){r=e instanceof TypeError?'TypeError':'other'}"
        ),
        "TypeError"
    );
}

/** A primitive locale is converted through its wrapper, so Number.prototype's indexed locales are consulted. */
#[test]
fn locale_argument_reads_indexed_properties_of_its_wrapper() {
    assert_eq!(
        eval(
            "Number.prototype.__proto__=[42];let r;try{'I'.toLocaleLowerCase(1);r='no throw'}catch(e){r=e instanceof TypeError?'TypeError':'other'}"
        ),
        "TypeError"
    );
}

/** An empty locales list is valid and leaves the default casing unchanged. */
#[test]
fn locale_case_accepts_empty_locale_list() {
    assert_eq!(
        eval("let r='AB'.toLocaleLowerCase([])+','+'ab'.toLocaleUpperCase();"),
        "ab,AB"
    );
}

/** The non-locale methods never read their argument, so a bad tag is harmless there. */
#[test]
fn plain_case_methods_ignore_locale_argument() {
    assert_eq!(
        eval("let r='AB'.toLowerCase([42])+','+'ab'.toUpperCase([42]);"),
        "ab,AB"
    );
}
