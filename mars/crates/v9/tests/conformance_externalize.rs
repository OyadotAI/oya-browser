/* Externalization host API: contents, Latin-1 reporting and argument validation, observed through real scripts. */
#![cfg(feature = "conformance")]
mod common;
use mars_v9::PageEnvironment;

/** Run one synchronous script and return its console lines; no timers or host tasks are involved. */
fn run(source: &str) -> Vec<String> {
    let sources = vec![source.to_string()];
    let host = common::host();
    let environment = PageEnvironment {
        conformance_d8: true,
        ..common::environment()
    };
    let execution = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    common::console(&execution)
}

#[test]
/** Externalizing leaves length, code units, well-formedness and concatenation exactly as they were. */
fn externalized_string_keeps_contents_and_semantics() {
    assert_eq!(
        run(
            "const s=createExternalizableString('abc\\uD83D');const before=[s.length,s.charCodeAt(3),s.isWellFormed()].join(':');externalizeString(s);console.log([before,[s.length,s.charCodeAt(3),s.isWellFormed()].join(':'),s+'x'===createExternalizableString('abc\\uD83Dx')].join('|'));"
        ),
        ["4:55357:false|4:55357:false|true"]
    );
}

#[test]
/** Externalizing a deferred concatenation flattens it without altering its code units. */
fn externalized_concatenation_keeps_its_units() {
    assert_eq!(
        run(
            "let s='a'.repeat(2000)+'\\u1234';externalizeString(s);console.log([s.length,s.charCodeAt(2000),s.slice(1999)==='a\\u1234'].join(':'));"
        ),
        ["2001:4660:true"]
    );
}

#[test]
/** Repeated externalization of one string is accepted and changes nothing observable. */
fn repeated_externalization_is_a_no_op() {
    assert_eq!(
        run(
            "const s=createExternalizableString('xyz');console.log([externalizeString(s),externalizeString(s),s].join(':'));"
        ),
        ["::xyz"]
    );
}

#[test]
/** Only Latin-1 content reports one-byte; a single wider unit, however the string was built, does not. */
fn is_one_byte_reports_latin1_content() {
    assert_eq!(
        run(
            "console.log([isOneByteString('abc'),isOneByteString('\\u00e9\\u00ff'),isOneByteString('\\u0100'),isOneByteString('ab\\u1234'.slice(0,2)),isOneByteString('a'+'\\u1234'),isOneByteString(''),isOneByteString('\\uD83D')].join(':'));"
        ),
        ["true:true:false:true:false:true:false"]
    );
}

#[test]
/** A two-byte source whose wide part is sliced away is Latin-1 content and reports as one-byte. */
fn is_one_byte_follows_content_not_storage() {
    assert_eq!(
        run(
            "const wide=createExternalizableString('\\u1234'+'ab'.repeat(3));console.log([isOneByteString(wide),isOneByteString(wide.slice(1))].join(':'));"
        ),
        ["false:true"]
    );
}

#[test]
/** Non-string operands are refused with an ordinary Error, not coerced to text first. */
fn externalize_rejects_non_string_operands() {
    assert_eq!(
        run(
            "let coerced=false;try{externalizeString({toString(){coerced=true;return 'x'}})}catch(e){console.log([e instanceof Error,e.message,coerced].join(':'))}"
        ),
        ["true:First parameter to externalizeString() must be a string.:false"]
    );
}

#[test]
/** Calling without an operand fails the same way as a non-string operand. */
fn externalize_without_operand_throws() {
    assert_eq!(
        run("try{externalizeString()}catch(e){console.log(e instanceof Error)}"),
        ["true"]
    );
}

#[test]
/** An empty string is refused, because V8 never externalizes it; the refusal is a catchable Error. */
fn externalize_rejects_the_empty_string() {
    assert_eq!(
        run(
            "try{externalizeString('')}catch(e){console.log([e instanceof Error,e.message].join(':'))}"
        ),
        ["true:String is too short to be externalized."]
    );
}

#[test]
/** isOneByteString refuses non-string operands instead of reporting a guess about them. */
fn is_one_byte_rejects_non_string_operands() {
    assert_eq!(
        run(
            "try{isOneByteString(1)}catch(e){console.log([e instanceof Error,e.message].join(':'))}"
        ),
        ["true:isOneByteString() requires a single string argument."]
    );
}

#[test]
/** The host constants are numbers so padding loops in V8-style tests terminate on real lengths. */
fn externalization_minimums_are_numbers() {
    assert_eq!(
        run(
            "console.log([typeof kExternalStringMinOneByteLength,typeof kExternalStringMinTwoByteLength,'E='.padEnd(kExternalStringMinOneByteLength,'.')].join(':'));"
        ),
        ["number:number:E="]
    );
}

#[test]
/** The Latin-1 flag combines through concatenation in either order, so a wide operand always wins. */
fn is_one_byte_flag_survives_concatenation() {
    assert_eq!(
        run(
            "const a='x'.repeat(2000);const b=a+'\\u1234';console.log([isOneByteString(a),isOneByteString(a+'y'),isOneByteString(b),isOneByteString('\\u1234'+a)].join(':'));"
        ),
        ["true:true:false:false"]
    );
}
