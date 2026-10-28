/** Super-property updates retain one receiver, base and canonical key through conversion, cleanup and suspension. */
mod common;

/** A returned arrow retains the original constructor, new.target, receiver and field initialization. */
#[test]
fn returned_arrow_initializes_its_lexical_constructor() {
    let source = "let seen;class Base{constructor(x){seen=new.target;this.x=x}}class Derived extends Base{field=this.x+1;constructor(x){return ()=>super(x)}}const run=new Derived(4);const obj=run();let twice=false;try{run()}catch(e){twice=e instanceof ReferenceError}const r=[obj.x,obj.field,seen===Derived,twice].join();";
    assert_eq!(common::eval(source, "r"), "4,5,true,true");
}

/** Reentrant parent construction checks the captured receiver only after the parent returns. */
#[test]
fn lexical_super_checks_for_reentrant_initialization_after_parent_returns() {
    let source = "let run,n=0,inner;class Base{constructor(){n++;if(n===1)inner=run()}}class Derived extends Base{field=7;constructor(){return ()=>super()}}run=new Derived();let failed=false;try{run()}catch(e){failed=e instanceof ReferenceError}const r=[n,inner.field,failed].join();";
    assert_eq!(common::eval(source, "r"), "2,7,true");
}

/** A lexical arrow invoked while its constructor is active initializes that constructor's receiver. */
#[test]
fn lexical_super_updates_an_active_constructor() {
    let source = "class Base{constructor(){this.x=3}}class Derived extends Base{field=this.x+1;constructor(){ {let run=()=>super();run()} this.y=eval('this.x')}}const obj=new Derived();const r=[obj.x,obj.field,obj.y].join();";
    assert_eq!(common::eval(source, "r"), "3,4,3");
}

/** Direct eval inside a returned arrow retains the same lexical constructor environment. */
#[test]
fn delayed_eval_super_uses_its_lexical_constructor() {
    let source = "class Base{constructor(){this.x=3}}class Derived extends Base{field=4;constructor(){return ()=>eval('super()')}}const run=new Derived();const obj=run();const r=[obj.x,obj.field].join();";
    assert_eq!(common::eval(source, "r"), "3,4");
}

/** Captured constructor state remains available after page state is serialized and restored. */
#[test]
fn returned_async_super_arrow_survives_state_transport() {
    let sources = vec!["class Base{constructor(x){this.x=x}}class Derived extends Base{field=this.x+1;constructor(x){return async()=>super(x)}}const run=new Derived(4);document.getElementById('go').addEventListener('click',()=>run().then(x=>console.log(x.x,x.field)));".into()];
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
    assert_eq!(common::console(&clicked), ["4 5"]);
}

/** Compound assignments retain their receiver, base, key and getter/setter ordering. */
#[test]
fn super_compound_assignment_keeps_one_captured_reference() {
    let source = "let keys=0;const log=[];const later={set x(v){throw Error('wrong')}};const base={get x(){log.push('get');Object.setPrototypeOf(home,later);return 4},set x(v){log.push('set');this.result=v}};const key={[Symbol.toPrimitive](){keys++;return 'x'}};const home={__proto__:base,m(){return super[key]+=(log.push('rhs'),3)}};const result=home.m();const r=[result,home.result,keys,log.join(':')].join();";
    assert_eq!(common::eval(source, "r"), "7,7,1,get:rhs:set");
}

/** Logical assignments skip RHS and setter when appropriate, without leaking reference operands. */
#[test]
fn super_logical_assignments_short_circuit_and_keep_one_reference() {
    let source = "const log=[];let current=0;const base={get x(){log.push('get');return current},set x(v){log.push('set');current=v}};const key={[Symbol.toPrimitive](){log.push('key');return 'x'}};const home={__proto__:base,m(){const a=super[key]&&=(log.push('bad'),1);const b=super[key]||=(log.push('rhs'),3);const c=super[key]??=(log.push('bad2'),4);return [a,b,c]}};const values=home.m();const r=[values.join(':'),current,log.join(':')].join('|');";
    assert_eq!(
        common::eval(source, "r"),
        "0:3:3|3|key:get:key:get:rhs:set:key:get"
    );
}

/** Named and computed postfix updates write on the actual receiver while leaving inherited data unchanged. */
#[test]
fn super_updates_use_the_method_receiver_for_accessors_and_data() {
    let source = "let correct=true;const base={a:100,b_:200,get b(){correct=correct&&this===home;return this.b_},set b(v){correct=correct&&this===home;this.b_=v}};const home={__proto__:base,m(){return [super.a++,super['b']++]}};const values=home.m();const r=[values.join(':'),home.a,home.b_,base.a,base.b_,correct].join();";
    assert_eq!(common::eval(source, "r"), "100:200,101,201,100,200,true");
}

/** Postfix returns the numeric old value, while prefix returns the written value after ToNumeric. */
#[test]
fn super_prefix_and_postfix_results_are_numeric() {
    let source = "const base={get x(){return this.value},set x(v){this.value=v}};const home={__proto__:base,value:'4',post(){return super.x++},pre(){return --super.x}};const old=home.post(),updated=home.value,now=home.pre();const zero={__proto__:{x:-0},m(){return super.x++}};const r=[old,typeof old,updated,now,home.value,Object.is(zero.m(),-0),zero.x].join();";
    assert_eq!(common::eval(source, "r"), "4,number,5,4,4,true,1");
}

/** The ordinary numeric update machinery preserves arbitrary-width BigInts instead of converting them to Number. */
#[test]
fn super_updates_preserve_bigint_width_and_type() {
    let source = "const start=1n<<200n;const base={get x(){return this.value},set x(v){this.value=v}};const home={__proto__:base,value:start,post(){return super.x++},pre(){return --super.x}};const old=home.post(),incremented=home.value,now=home.pre();const r=[old===start,incremented===start+1n,now===start,typeof now].join();";
    assert_eq!(common::eval(source, "r"), "true,true,true,bigint");
}

/** Key expression effects precede base capture, but key coercion effects cannot change the captured base. */
#[test]
fn computed_super_base_is_captured_between_key_evaluation_and_coercion() {
    let source = "let evaluated=0,coerced=0;const first={x:1},chosen={x:5},late={x:50};const key={[Symbol.toPrimitive](hint){coerced++;if(hint!=='string')throw 'hint';Object.setPrototypeOf(home,late);return 'x'}};function expression(){evaluated++;Object.setPrototypeOf(home,chosen);return key}const home={__proto__:first,m(){return ++super[expression()]}};const result=home.m();const r=[result,home.x,chosen.x,evaluated,coerced,Object.getPrototypeOf(home)===late].join();";
    assert_eq!(common::eval(source, "r"), "6,6,5,1,1,true");
}

/** Getter and ToNumeric side effects cannot redirect the final write to a new home prototype. */
#[test]
fn getter_and_numeric_conversion_keep_the_original_super_base() {
    let source = "const log=[];const late={set x(v){throw 'wrong setter'}};const middle={set x(v){throw 'wrong setter'}};const numeric={[Symbol.toPrimitive](hint){log.push(hint);Object.setPrototypeOf(home,late);return 6}};const base={get x(){log.push('get');Object.setPrototypeOf(home,middle);return numeric},set x(v){log.push('set '+v);this.written=v}};const home={__proto__:base,m(){return super.x++}};const old=home.m();const r=[old,home.written,log.join(':'),Object.getPrototypeOf(home)===late].join();";
    assert_eq!(common::eval(source, "r"), "6,7,get:number:set 7,true");
}

/** A proxy is the captured base itself; get and set receive the actual receiver and the same Symbol key once. */
#[test]
fn proxy_super_bases_keep_traps_receiver_and_symbol_key() {
    let source = "const key=Symbol('x');let keys=0;const log=[];const target={get [key](){return this.value},set [key](v){this.value=v}};const proxy=new Proxy(target,{get(t,k,r){log.push('get '+(k===key)+' '+(r===home));return Reflect.get(t,k,r)},set(t,k,v,r){log.push('set '+(k===key)+' '+(r===home));return Reflect.set(t,k,v,r)},getPrototypeOf(){throw 'unexpected prototype lookup'}});const computed={[Symbol.toPrimitive](){keys++;return key}};const home={__proto__:proxy,value:10,m(){return ++super[computed]}};const result=home.m();const r=[result,home.value,keys,log.join(':')].join();";
    assert_eq!(
        common::eval(source, "r"),
        "11,11,1,get true true:set true true"
    );
}

/** Failed writes are ignored only for sloppy methods; class methods retain their strict TypeError behavior. */
#[test]
fn super_updates_preserve_strict_and_sloppy_write_failures() {
    let source = "const frozen=Object.freeze({x:1});const home={__proto__:frozen,m(){return ++super.x}};class C{m(){return ++super.x}}Object.setPrototypeOf(C.prototype,frozen);let strict=false;try{new C().m()}catch(error){strict=error instanceof TypeError}const result=home.m();const r=[result,home.x,Object.hasOwn(home,'x'),frozen.x,strict].join();";
    assert_eq!(common::eval(source, "r"), "2,1,false,1,true");
}

/** Rendering a failed super write must never invoke an author-controlled constructor getter. */
#[test]
fn failed_super_write_diagnostic_does_not_call_constructor_getter() {
    let source = "let calls=0;const proto={};Object.defineProperty(proto,'constructor',{get(){calls++;throw 1}});const receiver=Object.create(proto);Object.defineProperty(receiver,'x',{value:1});const home={__proto__:{},m(){'use strict';super.x=2}};let type=false;try{home.m.call(receiver)}catch(e){type=e instanceof TypeError}const r=[type,calls,receiver.x].join();";
    assert_eq!(common::eval(source, "r"), "true,0,1");
}

/** A null captured super base throws before converting its key. */
#[test]
fn null_super_bases_keep_computed_key_conversion_order() {
    let source = "let keys=0;const marker={};const key={[Symbol.toPrimitive](){keys++;throw marker}};const home={__proto__:null,m(k){return super[k]++}};let same=false,type=false;try{home.m(key)}catch(error){same=error===marker}try{home.m({toString(){keys++;return 'x'}})}catch(error){type=error instanceof TypeError}const r=[same,type,keys].join();";
    assert_eq!(common::eval(source, "r"), "false,true,0");
}

/** Simple super assignment captures the base before the RHS and coerces the key afterwards. */
#[test]
fn simple_super_assignment_captures_base_and_defers_key_coercion() {
    let source = "const log=[];const first={set x(v){log.push('first:'+v)}},later={set x(v){log.push('later:'+v)}};const key={toString(){log.push('key');return 'x'}};const home={__proto__:first,m(){super[key]=(Object.setPrototypeOf(home,later),log.push('rhs'),7)}};home.m();const r=log.join();";
    assert_eq!(common::eval(source, "r"), "rhs,key,first:7");
}

/** A simple super read captures its base before coercion and checks null before the key. */
#[test]
fn simple_super_read_captures_base_before_key_coercion() {
    let source = "let count=0;const first={x:1},later={x:9};const key={toString(){count++;Object.setPrototypeOf(home,later);return 'x'}};const home={__proto__:first,m(){return super[key]}};const result=home.m();const empty={__proto__:null,m(){return super[key]}};let type=false;try{empty.m()}catch(e){type=e instanceof TypeError}const r=[result,count,type].join();";
    assert_eq!(common::eval(source, "r"), "1,1,true");
}

/** Deleting a super reference evaluates its key expression but never coerces the key. */
#[test]
fn delete_super_reference_throws_after_key_expression_without_coercion() {
    let source = "let expressions=0,coercions=0;const key={toString(){coercions++;throw 1}};class C{m(){delete super[(expressions++,key)]}}Object.setPrototypeOf(C.prototype,null);let reference=false;try{C.prototype.m()}catch(e){reference=e instanceof ReferenceError}const r=[reference,expressions,coercions].join();";
    assert_eq!(common::eval(source, "r"), "true,1,0");
}

/** Uninitialized derived this is rejected before evaluating a computed key or invoking a super constructor. */
#[test]
fn super_updates_check_this_before_computed_key_evaluation() {
    let source = "let bases=0,keys=0;class Base{constructor(){bases++;throw 'base'}}class Derived extends Base{constructor(){super[(keys++,super())]++}}let reference=false;try{new Derived()}catch(error){reference=error instanceof ReferenceError}const r=[reference,bases,keys].join();";
    assert_eq!(common::eval(source, "r"), "true,0,0");
}

/** Abrupt ToNumeric does not write, and key-conversion exceptions stop the getter before any read. */
#[test]
fn super_update_conversion_errors_prevent_later_observations() {
    let source = "let gets=0,sets=0;const marker={};const base={get x(){gets++;return {[Symbol.toPrimitive](){throw marker}}},set x(value){sets++}};const home={__proto__:base,m(key){return super[key]++}};let numeric=false,key=false;try{home.m('x')}catch(error){numeric=error===marker}try{home.m({toString(){throw marker}})}catch(error){key=error===marker}const r=[numeric,key,gets,sets].join();";
    assert_eq!(common::eval(source, "r"), "true,true,1,0");
}

/** Strict method receivers stay primitive when inherited accessors can accept them. */
#[test]
fn strict_super_updates_preserve_primitive_receivers() {
    let source = "let seen;class Base{get x(){return '3'}set x(value){seen=[this,value]}}class C extends Base{m(){return super.x++}}const result=C.prototype.m.call(7);const r=[result,seen[0],seen[1],typeof seen[0]].join();";
    assert_eq!(common::eval(source, "r"), "3,7,4,number");
}

/** Real collection during numeric conversion retains an otherwise unreachable original base until PutValue finishes. */
#[test]
fn captured_super_bases_remain_rooted_across_collection() {
    let source = "let written=0;let base={get x(){return {[Symbol.toPrimitive](){Object.setPrototypeOf(home,{});if(typeof gc==='function')gc();return 1}}},set x(value){written=value}};const home={__proto__:base,m(){return ++super.x}};base=null;const r=[home.m(),written].join();";
    assert_eq!(common::eval(source, "r"), "2,2");
}

/** Yielding the key saves the receiver, and base capture occurs only after that key expression resumes. */
#[test]
fn generator_super_update_keys_survive_state_transport() {
    let sources=vec!["const home={__proto__:{x:2},*m(){return super[yield 'key']++}};const gen=home.m();gen.next();document.getElementById('go').addEventListener('click',()=>{Object.setPrototypeOf(home,{x:9});const result=gen.next('x');console.log(result.value,result.done,home.x)});".into()];
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
    assert_eq!(common::console(&clicked), ["9 true 10"]);
}

/** Awaiting the computed key preserves the receiver through native jobs and JSON, without capturing the base too early. */
#[test]
fn async_super_update_keys_survive_state_transport() {
    let sources=vec!["let deliver;const wait=new Promise(resolve=>deliver=resolve);const home={__proto__:{x:2},async m(){return ++super[await wait]}};home.m().then(value=>console.log(value,home.x));document.getElementById('go').addEventListener('click',()=>{Object.setPrototypeOf(home,{x:9});deliver('x')});".into()];
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
    assert_eq!(common::console(&clicked), ["10 10"]);
}
