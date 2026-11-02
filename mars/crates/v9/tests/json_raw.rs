/** Raw JSON preserves a validated primitive spelling after toJSON and replacer callbacks. */
use crate::common;

/** Big decimal text does not lose digits through binary64 serialization. */
#[test]
fn raw_scalar_sources_are_serialized_verbatim() {
    assert_eq!(
        common::eval(
            "const r=JSON.stringify({n:JSON.rawJSON('123456789012345678901234567890'),s:JSON.rawJSON('\"x\"'),z:JSON.rawJSON('-0')});",
            "r"
        ),
        "{\"n\":123456789012345678901234567890,\"s\":\"x\",\"z\":-0}"
    );
}

/** Slots belong to the actual frozen object, while transparent proxies serialize as ordinary objects. */
#[test]
fn raw_objects_are_frozen_and_proxy_wrapping_does_not_copy_the_brand() {
    assert_eq!(
        common::eval(
            "const raw=JSON.rawJSON('true');const proxy=new Proxy(raw,{});const r=[Object.getPrototypeOf(raw)===null,Object.isFrozen(raw),JSON.isRawJSON(raw),JSON.isRawJSON(proxy),JSON.stringify(proxy)].join('|');",
            "r"
        ),
        "true|true|true|false|{\"rawJSON\":\"true\"}"
    );
}

/** Whitespace, containers and malformed sources cannot be used to inject arbitrary JSON fragments. */
#[test]
fn raw_creation_rejects_nonprimitive_or_nonexact_input() {
    assert_eq!(
        common::eval(
            "let errors=0;for(const text of ['', ' true', 'true ', '{}', '[]', '1,2', '01']){try{JSON.rawJSON(text)}catch(e){if(e instanceof SyntaxError)errors++}}const r=errors;",
            "r"
        ),
        "7"
    );
}

/** Replacer-produced raw values are recognized after the callback runs. */
#[test]
fn replacers_can_return_raw_json() {
    assert_eq!(
        common::eval(
            "const r=JSON.stringify({value:1},(key,value)=>key==='value'?JSON.rawJSON('1e999'):value);",
            "r"
        ),
        "{\"value\":1e999}"
    );
}

/** Raw JSON carries a non-serializable internal slot and cannot become a normal clone accidentally. */
#[test]
fn raw_json_is_not_structured_cloneable() {
    assert_eq!(
        common::eval(
            "let name='';try{structuredClone(JSON.rawJSON('1'))}catch(e){name=e.name}const r=name;",
            "r"
        ),
        "DataCloneError"
    );
}
