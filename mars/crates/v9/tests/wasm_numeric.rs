/** Wasm float conversions preserve payloads while quieting signaling NaNs. */
mod common;

/** Promoting an f32 signaling NaN sets the binary64 quiet bit without discarding its payload. */
#[test]
fn promote_f32_quiets_signaling_nan() {
    let source = "const b=new Uint8Array([0,97,115,109,1,0,0,0,1,5,1,96,0,1,127,3,2,1,0,7,5,1,1,102,0,0,10,15,1,13,0,67,0,0,160,127,187,189,66,32,136,167,11]);let r=new WebAssembly.Instance(new WebAssembly.Module(b)).exports.f()>>>0";
    assert_eq!(common::eval(source, "r"), "2147221504");
}

/** Division overflow and zero remainder have distinct observable trap messages. */
#[test]
fn integer_arithmetic_reports_its_actual_trap() {
    let source = "function f(op){const b=new Uint8Array([0,97,115,109,1,0,0,0,1,7,1,96,2,127,127,1,127,3,2,1,0,7,5,1,1,102,0,0,10,9,1,7,0,32,0,32,1,op,11]);return new WebAssembly.Instance(new WebAssembly.Module(b)).exports.f}let r='';try{f(109)(-2147483648,-1)}catch(e){r=e.message}try{f(111)(1,0)}catch(e){r+=','+e.message}";
    assert_eq!(
        common::eval(source, "r"),
        "divide result unrepresentable,remainder by zero"
    );
}
