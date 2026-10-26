/** Super-property updates retain one receiver, base and canonical key through conversion, cleanup and suspension. */
mod common;

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

/** A null super base is still captured before key conversion, whose own exception has precedence. */
#[test]
fn null_super_bases_keep_computed_key_conversion_order() {
    let source = "let keys=0;const marker={};const key={[Symbol.toPrimitive](){keys++;throw marker}};const home={__proto__:null,m(k){return super[k]++}};let same=false,type=false;try{home.m(key)}catch(error){same=error===marker}try{home.m({toString(){keys++;return 'x'}})}catch(error){type=error instanceof TypeError}const r=[same,type,keys].join();";
    assert_eq!(common::eval(source, "r"), "true,true,2");
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
