/** Optional-chain values keep successful member references through parentheses and erase failed references. */
mod common;

/** Parenthesized optional members preserve the object receiver for calls and computed-name calls. */
#[test]
fn parenthesized_optional_members_preserve_their_receivers() {
    let source = "const o={value:42,m(){return this.value}},p={o};const r=[(o?.m)(),(o?.['m'])(),(p?.o?.m)(),(p?.['o']?.['m'])(),(o?.m)?.()].join();";
    assert_eq!(common::eval(source, "r"), "42,42,42,42,42");
}

/** Short-circuiting within parentheses finishes before the outer call, including its argument effects. */
#[test]
fn short_circuited_parenthesized_calls_still_evaluate_outer_arguments() {
    let source = "let count=0,threw=false;try{(null?.m)(++count)}catch(e){threw=e instanceof TypeError}const skipped=(null?.m)?.(++count);const r=[count,threw,skipped].join();";
    assert_eq!(common::eval(source, "r"), "1,true,");
}

/** A deleted nullish optional reference is true, while computed keys and arguments remain unevaluated. */
#[test]
fn short_circuited_delete_returns_true_and_skips_effects() {
    let source = "let count=0;const o={value:1};const r=[delete null?.m,delete undefined?.[++count],delete null?.(++count),delete o?.value,'value' in o,count].join();";
    assert_eq!(common::eval(source, "r"), "true,true,true,true,false,0");
}

/** Optional method calls retain exactly one getter and key evaluation on their successful path. */
#[test]
fn parenthesized_reference_getters_are_evaluated_once() {
    let source = "let reads=0,keys=0;const o={value:9,get m(){reads++;return function(){return this.value}}};const value=(o?.[(keys++,'m')])();const r=[value,reads,keys].join();";
    assert_eq!(common::eval(source, "r"), "9,1,1");
}

/** Private member references carry the same successful receiver as ordinary optional members. */
#[test]
fn parenthesized_optional_private_references_preserve_the_receiver() {
    let source = "class C{static #self=C;static #method(){return this}static test(){return [(C?.#method)()===C,(C?.#self?.#method)()===C,(C?.#method)?.()===C].join()}}const r=C.test();";
    assert_eq!(common::eval(source, "r"), "true,true,true");
}

/** Parentheses also preserve the receiver when an optional member becomes a template tag. */
#[test]
fn parenthesized_optional_tags_preserve_their_receivers() {
    let source = "const o={value:7,tag(){return this.value}};const r=(o?.tag)`text`;";
    assert_eq!(common::eval(source, "r"), "7");
}
