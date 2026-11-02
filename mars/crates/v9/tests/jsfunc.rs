/** Function objects, %ThrowTypeError% and private-name brand checks, through the page API. */
use crate::common;
use common::eval;

/** SetFunctionName/SetFunctionLength make ordinary configurable properties: delete sticks. */
#[test]
fn function_name_and_length_delete_and_redefine() {
    let source = "function f(a,b){} delete f.name; delete f.length;\
        const gone=[f.hasOwnProperty('name'), f.hasOwnProperty('length'), Object.getOwnPropertyNames(f).includes('name')].join();\
        Object.defineProperty(f,'name',{value:'g',configurable:true});\
        delete Math.max.name; const native=Math.max.hasOwnProperty('name');\
        const r=[gone, f.name, native].join('|');";
    assert_eq!(eval(source, "r"), "false,false,false|g|false");
}

/** Function.prototype.caller/arguments and a strict arguments.callee share one frozen thrower. */
#[test]
fn restricted_properties_throw_type_error() {
    let source = "'use strict'; function s(){ return arguments }\
        const callee=Object.getOwnPropertyDescriptor(s(),'callee').get;\
        const fp=Object.getOwnPropertyDescriptor(Function.prototype,'caller');\
        let threw=false; try{ s.caller }catch(e){ threw=e instanceof TypeError }\
        const nd=Object.getOwnPropertyDescriptor(callee,'name');\
        const r=[threw, fp.get===callee, fp.set===callee, fp.configurable, nd.configurable, Object.isExtensible(callee)].join();";
    assert_eq!(eval(source, "r"), "true,true,true,true,false,false");
}

/** Sloppy functions keep V8's null `caller` and a mapped `arguments.callee`. */
#[test]
fn sloppy_functions_read_null_caller() {
    let source = "function f(){ return arguments.callee===f } const r=[f.caller, f(), (()=>1).hasOwnProperty('caller')].join();";
    assert_eq!(eval(source, "r"), ",true,false");
}

/** PrivateGet/PrivateSet on an object without the name, or a write to a private method, throw. */
#[test]
fn private_names_check_their_brand() {
    let source = "class C{ #f=1; #m(){} get(o){ return o.#f } set(o){ o.#f=2 } wm(){ this.#m=1 } }\
        const c=new C(); const out=[];\
        for (const run of [()=>c.get({}), ()=>c.set({}), ()=>c.wm(), ()=>c.get(5)]) {\
          try{ run(); out.push('ok') }catch(e){ out.push(e instanceof TypeError) } }\
        out.push(c.get(c)); c.set(c); out.push(c.get(c)); const r=out.join();";
    assert_eq!(eval(source, "r"), "true,true,true,true,1,2");
}

/** A function with a non-simple parameter list cannot also say "use strict", and a private
reference cannot be deleted: both are early errors that stop the whole script. */
#[test]
fn early_errors_for_strict_directive_and_private_delete() {
    for source in [
        "function f([a]){ 'use strict' }",
        "var f = (...a) => { 'use strict' }",
        "class C { #x; m(){ delete this.#x } }",
    ] {
        let err = mars_v9::check(source).expect_err(source);
        assert_eq!(err.code, "script_syntax_error", "{source}");
    }
    assert!(mars_v9::check("function f(a){ 'use strict' }").is_ok());
}

/** BoundFunctionCreate takes `length` only from the target's own property. */
#[test]
fn bound_length_ignores_inherited_length() {
    let source = "function bar(a,b){} Object.setPrototypeOf(bar,{length:42}); delete bar.length;\
        const r=[Function.prototype.bind.call(bar,null,1).length, (function(a,b,c){}).bind(null,1).length].join();";
    assert_eq!(common::eval(source, "r"), "0,2");
}
