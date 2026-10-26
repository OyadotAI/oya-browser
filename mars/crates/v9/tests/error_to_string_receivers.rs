/** Error string conversion rejects primitive receivers with inert diagnostics before reading ordinary object fields. */
mod common;

/** Rejected primitives retain their actual display without boxing or authored conversion. */
#[test]
fn primitive_receivers_report_the_incompatible_method_and_value() {
    let source = "const prefix='Method Error.prototype.toString called on incompatible receiver ';const values=['invalid',23,undefined,null,true,0,-0,NaN,Infinity,12n,Symbol('key')];const expected=['invalid','23','undefined','null','true','0','0','NaN','Infinity','12','Symbol(key)'];const r=values.every((value,index)=>{try{Error.prototype.toString.call(value);return false}catch(error){return error instanceof TypeError&&error.message===prefix+expected[index]}});";
    assert_eq!(common::eval(source, "r"), "true");
}

/** Native diagnostic rendering does not consult a primitive wrapper's conversion properties. */
#[test]
fn rejected_primitives_do_not_run_wrapper_conversion_hooks() {
    let source = "let calls=0;for(const prototype of [String.prototype,Number.prototype,Boolean.prototype,BigInt.prototype,Symbol.prototype]){prototype.toString=function(){calls++;throw 1};prototype.valueOf=function(){calls++;throw 2};prototype[Symbol.toPrimitive]=function(){calls++;throw 3}}let types=0;for(const value of ['invalid',23,true,12n,Symbol('key')]){try{Error.prototype.toString.call(value)}catch(error){if(error instanceof TypeError)types++}}const r=types===5&&calls===0;";
    assert_eq!(common::eval(source, "r"), "true");
}

/** Accepted objects keep the observable name-then-message lookup and coercion order. */
#[test]
fn object_receivers_keep_normal_name_and_message_evaluation() {
    let source = "const log=[];const value={get name(){log.push('name');return {toString(){log.push('name string');return 'Kind'}}},get message(){log.push('message');return {toString(){log.push('message string');return 'detail'}}}};const text=Error.prototype.toString.call(value);const r=[text,log.join(':')].join('|');";
    assert_eq!(
        common::eval(source, "r"),
        "Kind: detail|name:name string:message:message string"
    );
}

/** The first failed object lookup retains exception identity and prevents later getters. */
#[test]
fn accepted_object_lookup_exceptions_keep_their_identity() {
    let source = "const marker={};let messageReads=0,same=false;try{Error.prototype.toString.call({get name(){throw marker},get message(){messageReads++;return 'unused'}})}catch(error){same=error===marker}const r=same&&messageReads===0;";
    assert_eq!(common::eval(source, "r"), "true");
}
