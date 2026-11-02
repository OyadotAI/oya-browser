/** Property references preserve native receiver brands, null-base error ordering and current Test262 key/with semantics. */
use crate::common;

/** Object environment records require HasBinding and a second GetBindingValue HasProperty observation. */
#[test]
fn with_proxy_get_performs_both_specified_has_checks() {
    let source = "const log=[];const proxy=new Proxy({x:1},{has(target,key){log.push('has:'+String(key));return Reflect.has(target,key)},get(target,key,receiver){log.push('get:'+String(key));return Reflect.get(target,key,receiver)}});let actual;with(proxy){actual=x}const r=[actual,log.filter(item=>!item.includes('actual')).join('|')].join(';');";
    assert_eq!(
        common::eval(source, "r"),
        "1;has:x|get:Symbol(Symbol.unscopables)|has:x|get:x"
    );
}

/** Deletion during the unscopables getter cannot resurrect a vanished binding or call its value getter. */
#[test]
fn with_deleted_binding_reads_undefined_after_unscopables() {
    let source = "let called=0;const env={get x(){called++;return 1},get [Symbol.unscopables](){delete this.x;return null}};let result;with(env){result=x}const r=[result,called].join();";
    assert_eq!(common::eval(source, "r"), ",0");
}

/** Strict super stores propagate OrdinarySet failure instead of returning success for primitive or readonly receivers. */
#[test]
fn strict_super_stores_reject_failed_receiver_writes() {
    let source = "class C{store(){return super.x=10}}const out=[];for(const receiver of [undefined,null,42,'abc']){try{C.prototype.store.call(receiver);out.push('allowed')}catch(e){out.push(e.name)}}const receiver={};out.push(C.prototype.store.call(receiver),receiver.x);Object.defineProperty(Object.prototype,'x',{value:1,configurable:true});try{C.prototype.store.call({})}catch(e){out.push(e.name)}const r=out.join();";
    assert_eq!(
        common::eval(source, "r"),
        "TypeError,TypeError,TypeError,TypeError,10,10,TypeError"
    );
}

/** An inherited accessor receives the original primitive receiver; it may legitimately accept that receiver. */
#[test]
fn strict_super_accessors_keep_the_actual_receiver() {
    let source = "let seen;const base={set x(value){'use strict';seen=[this,value].join()}};class C{store(){return super.x=9}}Object.setPrototypeOf(C.prototype,base);const value=C.prototype.store.call(42);const r=[value,seen].join('|');";
    assert_eq!(common::eval(source, "r"), "9|42,9");
}

/** A sloppy method's failed super store remains silent, while null super bases still throw. */
#[test]
fn sloppy_super_writes_observe_reference_strictness() {
    let source = "const base={};Object.defineProperty(base,'x',{value:1});const object={__proto__:base,store(){return super.x=2}};const out=[object.store(),Object.hasOwn(object,'x')];Object.setPrototypeOf(object,null);try{object.store()}catch(e){out.push(e.name)}const r=out.join();";
    assert_eq!(common::eval(source, "r"), "2,false,TypeError");
}

/** Primitive strings own readonly UTF16 indices and length, so only out-of-range prototype setters run. */
#[test]
fn string_primitive_own_properties_block_prototype_setters() {
    let source = "const out=[];for(const name of ['0','1','3'])Object.defineProperty(String.prototype,name,{set(value){out.push(name+':'+value)},configurable:true});let string='abc';string[0]=1;string[3]=2;let pair='💩';pair[1]=3;let error;try{(function(){'use strict';string[1]=4})()}catch(e){error=e.name}const r=[out.join(),error].join('|');";
    assert_eq!(common::eval(source, "r"), "3:2|TypeError");
}

/** String.prototype itself is the empty string wrapper, including its immutable own length data property. */
#[test]
fn string_prototype_length_is_an_immutable_own_property() {
    let source = "const descriptor=Object.getOwnPropertyDescriptor(String.prototype,'length');Number.prototype.__proto__=String.prototype;const r=[String.prototype.length,(123).length,descriptor.value,descriptor.writable,descriptor.enumerable,descriptor.configurable].join();";
    assert_eq!(common::eval(source, "r"), "0,0,0,false,false,false");
}

/** Key expression evaluation precedes null-base failure, but ToPropertyKey never runs for that null receiver. */
#[test]
fn computed_null_access_rejects_before_key_coercion() {
    let source = "const out=[];const key={toString(){out.push('coerce');return 'x'}};function expression(){out.push('expression');return key}for(const run of [()=>null[expression()],()=>undefined[expression()],()=>{null[expression()]+=out.push('rhs')},()=>{undefined[expression()]++},()=>{null[expression()]||=out.push('rhs')},()=>{delete null[expression()]}]){try{run()}catch(e){out.push(e.name)}}const r=out.join();";
    assert_eq!(
        common::eval(source, "r"),
        "expression,TypeError,expression,TypeError,expression,TypeError,expression,TypeError,expression,TypeError,expression,TypeError"
    );
}

/** Simple assignment converts a computed key after the RHS and rejects a null base before that conversion. */
#[test]
fn simple_computed_assignment_defers_key_coercion_until_putvalue() {
    let source = "const out=[];const key={toString(){out.push('coerce');return 'x'}};const object={};object[(out.push('expression'),key)]=(out.push('rhs'),1);try{null[(out.push('null-expression'),key)]=(out.push('null-rhs'),2)}catch(e){out.push(e.name)}const r=[out.join(),object.x].join('|');";
    assert_eq!(
        common::eval(source, "r"),
        "expression,rhs,coerce,null-expression,null-rhs,TypeError|1"
    );
}

/** Compound, logical and update references retain one canonical key even if the RHS changes coercion hooks. */
#[test]
fn read_modify_write_references_canonicalize_the_key_only_once() {
    let source = "const out=[];const object={x:2,y:100};let count=0;const key={toString(){count++;return count===1?'x':'y'}};object[key]+=(out.push('rhs'),3);out.push(count,object.x,object.y);count=0;object[key]&&=(key.toString=()=>{count++;return 'y'},9);out.push(count,object.x,object.y);count=0;key.toString=()=>{count++;return 'x'};const old=object[key]++;out.push(count,old,object.x);const r=out.join();";
    assert_eq!(common::eval(source, "r"), "rhs,1,5,100,1,9,100,1,9,10");
}

/** Computed reference key objects remain genuine graph edges when simple assignment's RHS awaits a promise. */
#[test]
fn delayed_reference_keys_survive_state_transport() {
    let sources=vec!["const object={};let resolve;const wait=new Promise(done=>resolve=done);let name='before';const key={toString(){return name}};async function assign(){object[key]=await wait;console.log(object.after,object.before)}assign();document.getElementById('go').addEventListener('click',()=>{name='after';resolve(7)});".into()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let clicked = mars_v9::click_page(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        3,
    )
    .unwrap();
    assert!(clicked.failed.is_empty(), "{:?}", common::failure(&clicked));
    assert_eq!(common::console(&clicked), ["7 undefined"]);
}
