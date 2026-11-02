/** String splitting and number formatting preserve authored conversion order even when a shortcut returns immediately. */
use crate::common;

/** A zero split limit skips searching, but still observes receiver, limit and separator conversions in order. */
#[test]
fn split_zero_limit_still_converts_the_separator() {
    let source = "let log='';const receiver={toString(){log+='receiver;';return 'subject'}};const separator={get [Symbol.split](){log+='split;';return undefined},toString(){log+='separator;';return 'b'}};const limit={valueOf(){log+='limit;';return 0}};const result=String.prototype.split.call(receiver,separator,limit);const r=[log,result.length,Array.isArray(result)].join('|');";
    assert_eq!(
        common::eval(source, "r"),
        "split;receiver;limit;separator;|0|true"
    );
}

/** Limit conversion failure propagates before a separator conversion can run. */
#[test]
fn split_limit_failure_precedes_separator_coercion() {
    let source = "let log='';const marker={};const separator={get [Symbol.split](){log+='split;';return undefined},toString(){log+='separator;';throw 1}};const receiver={toString(){log+='receiver;';return 'x'}};const limit={valueOf(){log+='limit;';throw marker}};let same=false;try{String.prototype.split.call(receiver,separator,limit)}catch(error){same=error===marker}const r=[same,log].join('|');";
    assert_eq!(common::eval(source, "r"), "true|split;receiver;limit;");
}

/** A separator conversion error remains observable at limit zero instead of returning an empty result first. */
#[test]
fn split_zero_limit_propagates_separator_conversion_failure() {
    let source = "const marker={};let conversions=0;const separator={toString(){conversions++;throw marker}};let same=false;try{'subject'.split(separator,0)}catch(error){same=error===marker}const r=[same,conversions,'subject'.split(undefined,0).length].join();";
    assert_eq!(common::eval(source, "r"), "true,1,0");
}

/** A custom split method gets the original receiver and limit without triggering the fallback string or number conversions. */
#[test]
fn custom_split_keeps_the_original_arguments() {
    let source = "let conversions=0;const receiver={toString(){conversions++;throw 1}};const limit={valueOf(){conversions++;throw 2}};const separator={[Symbol.split](value,bound){return value===receiver&&bound===limit},toString(){conversions++;throw 3}};const result=String.prototype.split.call(receiver,separator,limit);const r=[result,conversions].join();";
    assert_eq!(common::eval(source, "r"), "true,0");
}

/** Exponential and precision methods convert the digit argument before returning a nonfinite receiver's text. */
#[test]
fn nonfinite_number_formatting_still_converts_digits() {
    let source = "let hints='';const digits={[Symbol.toPrimitive](hint){hints+=hint+':';return 1000}};const out=[];for(const method of ['toExponential','toPrecision'])for(const value of [NaN,Infinity,-Infinity])out.push(Number.prototype[method].call(value,digits));const r=[out.join(),hints].join('|');";
    assert_eq!(
        common::eval(source, "r"),
        "NaN,Infinity,-Infinity,NaN,Infinity,-Infinity|number:number:number:number:number:number:"
    );
}

/** Abrupt digit conversions beat the nonfinite shortcut, including the native Symbol-to-number TypeError. */
#[test]
fn nonfinite_number_formatting_propagates_digit_conversion_errors() {
    let source = "const marker={};const digits={valueOf(){throw marker}};const out=[];for(const method of ['toExponential','toPrecision']){try{Number.prototype[method].call(NaN,digits)}catch(error){out.push(error===marker)}try{Number.prototype[method].call(Infinity,Symbol())}catch(error){out.push(error instanceof TypeError)}}const r=out.join();";
    assert_eq!(common::eval(source, "r"), "true,true,true,true");
}

/** Fixed notation checks the converted range before nonfinite text, while exponential and precision validate finite values only. */
#[test]
fn number_formatting_preserves_its_method_specific_range_order() {
    let source = "let calls=0;const digits={valueOf(){calls++;return 1000}};const out=[];for(const value of [NaN,Infinity,-Infinity]){try{Number.prototype.toFixed.call(value,digits)}catch(error){out.push(error instanceof RangeError)}}for(const method of ['toExponential','toPrecision']){try{Number.prototype[method].call(1,digits)}catch(error){out.push(error instanceof RangeError)}}out.push(calls);const r=out.join();";
    assert_eq!(common::eval(source, "r"), "true,true,true,true,true,5");
}

/** Receiver branding happens before digit conversion, and boxed Number values never call authored valueOf hooks. */
#[test]
fn number_formatting_uses_the_receiver_brand_before_digits() {
    let source = "let calls=0;const digits={valueOf(){calls++;return 2}};const boxed=new Number(1);boxed.valueOf=()=>{throw 1};const out=[];for(const method of ['toExponential','toPrecision','toFixed']){try{Number.prototype[method].call({},digits)}catch(error){out.push(error instanceof TypeError)}out.push(Number.prototype[method].call(boxed,digits))}out.push(calls);const r=out.join();";
    assert_eq!(
        common::eval(source, "r"),
        "true,1.00e+0,true,1.0,true,1.00,3"
    );
}
