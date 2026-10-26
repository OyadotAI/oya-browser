/** BigInt as real pages use it (Google's Closure int64 helpers), bounded by retained-value quotas. */
mod common;
use common::{eval, failure, load};

/** Literals, conversion and the 64-bit helpers Closure's int64 code runs at load. */
#[test]
fn closure_int64_helpers_run() {
    let source = "const lo=BigInt(Number.MIN_SAFE_INTEGER),hi=BigInt(Number.MAX_SAFE_INTEGER);\
        const r=[typeof 1n,String(lo),hi+1n,BigInt.asUintN(64,(BigInt(5>>>0)<<BigInt(32))+BigInt(7>>>0)),\
        BigInt.asIntN(64,0xffffffffffffffffn),BigInt.asUintN(64,-1n),BigInt('0x1f'),(255n).toString(16),\
        10n/3n,-7n%3n,2n**64n,-8n>>1n,1n==1,2n>1,'3'==3n,0n?1:2,[1n].includes(1n),Number(2n**53n)].join();";
    assert_eq!(
        eval(source, "r"),
        "bigint,-9007199254740991,9007199254740992,21474836487,-1,18446744073709551615,31,ff,3,-1,18446744073709551616,-4,true,true,true,2,true,9007199254740992"
    );
}

/** Mixing BigInt with Number, unsigned shifts and unary plus are TypeErrors, as in JavaScript. */
#[test]
fn mixing_types_throws_type_errors() {
    let source = "const r=[()=>1n+1,()=>1n>>>0n,()=>+1n,()=>JSON.stringify(1n),()=>Math.max(1n)].map(f=>{try{f();return 'ok'}catch(e){return e.name}}).join();";
    assert_eq!(
        eval(source, "r"),
        "TypeError,TypeError,TypeError,TypeError,TypeError"
    );
}

/** Oversized results and bad conversions are catchable RangeErrors and SyntaxErrors. */
#[test]
fn bounds_and_bad_conversions_throw_catchably() {
    let source = "const r=[()=>2n**(2n**64n),()=>1n<<(1n<<64n),()=>1n/0n,()=>BigInt(1.5),()=>BigInt('x')].map(f=>{try{f();return 'ok'}catch(e){return e.name}}).join();";
    assert_eq!(
        eval(source, "r"),
        "RangeError,RangeError,RangeError,RangeError,SyntaxError"
    );
}

/** BigInt values and wrappers survive the state wire between requests. */
#[test]
fn bigints_survive_the_state_wire() {
    let execution = load("var big=123456789012345678901234567890n;var boxed=Object(5n);");
    assert!(failure(&execution).is_none());
    let wire = serde_json::to_string(&execution.state).unwrap();
    let state: mars_v9::VmState = serde_json::from_str(&wire).unwrap();
    assert_eq!(serde_json::to_string(&state).unwrap(), wire);
}
