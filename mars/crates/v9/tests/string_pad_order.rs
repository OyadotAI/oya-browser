/** String padding coerces only operands needed after receiver length and requested target are known. */
mod common;

/** Public execution fails loudly on every unexpected author or native exception. */
fn result(source: &str) -> String {
    common::eval(source, "r")
}

/** Every target no greater than the existing UTF-16 length returns before consulting the filler. */
#[test]
fn unnecessary_padding_does_not_convert_throwing_fillers() {
    let actual = result(
        "let calls=0,passed=0;const filler={toString(){calls++;throw Error('unexpected filler')}};for(const name of ['padStart','padEnd'])for(const target of [NaN,-Infinity,-9,-1,-0,0,3,5]){if('hello'[name](target,filler)==='hello')passed++}const r=[passed,calls].join();",
    );
    assert_eq!(actual, "16,0");
}

/** Receiver coercion precedes numeric target coercion, while an unnecessary filler remains untouched. */
#[test]
fn receiver_and_target_coerce_in_order_before_the_early_return() {
    let actual = result(
        "const order=[];for(const name of ['padStart','padEnd']){const receiver={[Symbol.toPrimitive](hint){order.push('receiver:'+hint);return 'hello'}};const target={[Symbol.toPrimitive](hint){order.push('target:'+hint);return 3}};const filler={toString(){order.push('filler');throw 1}};String.prototype[name].call(receiver,target,filler)}const r=order.join();",
    );
    assert_eq!(
        actual,
        "receiver:string,target:number,receiver:string,target:number"
    );
}

/** A throwing receiver wins before target conversion; Symbol targets reject before touching a filler. */
#[test]
fn earlier_operand_errors_preserve_identity_and_skip_later_conversions() {
    let actual = result(
        "const marker={};let same=0,type=0,touches=0;for(const name of ['padStart','padEnd']){try{String.prototype[name].call({toString(){throw marker}},{valueOf(){touches++;return 9}},{toString(){touches++;return '.'}})}catch(e){if(e===marker)same++}try{''[name](Symbol(),{toString(){touches++;throw marker}})}catch(e){if(e instanceof TypeError)type++}}const r=[same,type,touches].join();",
    );
    assert_eq!(actual, "2,2,0");
}

/** Necessary padding converts its filler before deciding whether the result could exceed a resource bound. */
#[test]
fn necessary_fillers_throw_before_large_target_admission() {
    let actual = result(
        "const marker={};let calls=0,same=0;for(const name of ['padStart','padEnd'])for(const target of [9,2**31,2**32,2**53,Infinity]){try{'hello'[name](target,{toString(){calls++;throw marker}})}catch(e){if(e===marker)same++}}const r=[calls,same].join();",
    );
    assert_eq!(actual, "10,10");
}

/** An empty converted filler returns the original string even for an enormous requested target. */
#[test]
fn empty_fillers_avoid_allocation_for_large_requested_targets() {
    let actual = result(
        "let calls=0,same=0;for(const name of ['padStart','padEnd'])for(const target of [9,2**32,2**53,Infinity])for(const source of ['','hello']){const filler={toString(){calls++;return ''}};if(source[name](target,filler)===source)same++}const r=[calls,same].join();",
    );
    assert_eq!(actual, "16,16");
}

/** Symbol fillers are not converted for no-op padding, but still reject when padding is actually needed. */
#[test]
fn symbol_fillers_are_observed_only_when_padding_is_required() {
    let actual = result(
        "let noops=0,errors=0;for(const name of ['padStart','padEnd']){if('hello'[name](5,Symbol())==='hello')noops++;try{'hello'[name](6,Symbol())}catch(e){if(e instanceof TypeError)errors++}}const r=[noops,errors].join();",
    );
    assert_eq!(actual, "2,2");
}

/** Existing astral and lone-surrogate lengths are code units; normal cyclic padding preserves raw code units. */
#[test]
fn utf16_lengths_and_existing_padding_values_remain_exact() {
    let actual = result(
        "let touched=0;const filler={toString(){touched++;throw 1}};const astral='\\ud83d\\ude00',lone='\\ud800';const values=[astral.padStart(2,filler),astral.padEnd(1,filler),lone.padStart(1,filler),'x'.padStart(4,'ab'),'x'.padEnd(4,'ab')];const r=[values[0]===astral,values[1]===astral,values[2]===lone,values[3],values[4],touched].join();",
    );
    assert_eq!(actual, "true,true,true,abax,xaba,0");
}

/** No-op padding does not even read coercion hooks from a Proxy filler. */
#[test]
fn unnecessary_padding_avoids_proxy_coercion_property_traps() {
    let actual = result(
        "const log=[];const filler=new Proxy({},{get(target,key){log.push(String(key));throw 1}});const a='hello'.padStart(5,filler),b='hello'.padEnd(0,filler);const r=[a,b,log.length].join();",
    );
    assert_eq!(actual, "hello,hello,0");
}
