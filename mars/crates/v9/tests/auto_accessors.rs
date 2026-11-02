/** Auto-accessors use private per-instance storage and real prototype or static accessor descriptors. */
use crate::common;

/** Initializers create backing slots, while the public property remains a nonenumerable prototype accessor. */
#[test]
fn auto_accessors_store_per_instance_values_behind_prototype_accessors() {
    let source = "class C{accessor value=1;accessor missing}const a=new C,b=new C;a.value=7;const d=Object.getOwnPropertyDescriptor(C.prototype,'value');const r=[a.value,b.value,a.missing,Object.keys(a).length,a.hasOwnProperty('value'),d.enumerable,d.configurable,d.get.name,d.set.name,d.get.length,d.set.length].join();";
    assert_eq!(
        common::eval(source, "r"),
        "7,1,,0,false,false,true,get value,set value,0,1"
    );
}

/** Getters and setters check own backing brands and never inherit another receiver's private storage. */
#[test]
fn auto_accessor_functions_reject_unbranded_receivers() {
    let source = "class C{accessor value=1}const d=Object.getOwnPropertyDescriptor(C.prototype,'value'),instance=new C;let count=0;for(const target of [{},C.prototype,Object.create(instance),3]){try{d.get.call(target)}catch(e){if(e instanceof TypeError)count++}try{d.set.call(target,2)}catch(e){if(e instanceof TypeError)count++}}const r=count;";
    assert_eq!(common::eval(source, "r"), "8");
}

/** Public freezing does not freeze private backing slots, so an existing setter remains effective. */
#[test]
fn frozen_instances_keep_mutable_auto_accessor_backing() {
    let source = "class C{accessor value=1}const instance=Object.freeze(new C);instance.value=9;const r=instance.value;";
    assert_eq!(common::eval(source, "r"), "9");
}

/** Static and private auto-accessors install at their proper homes and initialize in declaration order. */
#[test]
fn static_and_private_auto_accessors_use_distinct_backing_brands() {
    let source = "const log=[];class C{accessor #value=(log.push('instance'),2);static accessor value=(log.push('static'),3);read(){return this.#value}write(value){this.#value=value}}const instance=new C;instance.write(8);C.value=7;class D extends C{}let denied=false;try{D.value}catch(e){denied=e instanceof TypeError}const r=[instance.read(),C.value,denied,log.join(':')].join();";
    assert_eq!(common::eval(source, "r"), "8,7,true,static:instance");
}

/** Computed names and coercion happen once at class definition before any instance initialization. */
#[test]
fn computed_auto_accessor_keys_are_captured_once_in_source_order() {
    let source = "const log=[];const symbol=Symbol('key');class C{accessor[(log.push('key'),symbol)]=(log.push('init'),4);[(log.push('method'),'method')](){}}log.push('defined');const a=new C,b=new C;a[symbol]=8;const d=Object.getOwnPropertyDescriptor(C.prototype,symbol);const r=[a[symbol],b[symbol],d.get.name,d.set.name,log.join(':')].join('|');";
    assert_eq!(
        common::eval(source, "r"),
        "8|4|get [key]|set [key]|key:method:defined:init:init"
    );
}

/** Backing keys minted by one class evaluation persist with both accessor functions through JSON transport. */
#[test]
fn auto_accessor_backings_survive_public_state_transport() {
    let sources=vec!["function make(){return class{accessor value=1}}const A=make(),B=make(),a=new A,b=new B;a.value=7;document.getElementById('go').addEventListener('click',()=>{a.value++;console.log(a.value,b.value,Object.getOwnPropertyDescriptor(A.prototype,'value').get!==Object.getOwnPropertyDescriptor(B.prototype,'value').get)});".to_string()];
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
    assert_eq!(common::console(&clicked), ["8 1 true"]);
}

/** Anonymous auto-accessor initializers receive the public computed name, rather than the hidden backing spelling. */
#[test]
fn computed_auto_accessor_initializers_receive_the_public_name() {
    let source = "const seen=[];class C{accessor ['method']=()=>{};accessor ['nested']=class{static{seen.push(this.name)}};static accessor ['factory']=function(){}}const value=new C;const r=[value.method.name,value.nested.name,C.factory.name,seen.join()].join();";
    assert_eq!(common::eval(source, "r"), "method,nested,factory,nested");
}
