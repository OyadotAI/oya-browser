/** Computed class field keys are captured during definition, separately from per-instance initializer values. */
mod common;

/** Computed keys run once in source order with methods, while values run for each new instance. */
#[test]
fn field_keys_run_once_during_definition_in_source_order() {
    let source = "const log=[];function key(n){log.push(n);return n}class C{[key('first')]=(log.push('value'),1);[key('method')](){}static [key('static')]=2;[key('last')]=3}log.push('defined');const a=new C,b=new C;const r=[log.join(':'),a.first,b.last,C.static,typeof a.method].join('|');";
    assert_eq!(
        common::eval(source, "r"),
        "first:method:static:last:defined:value:value|1|3|2|function"
    );
}

/** Key conversion uses the outer this once, instead of the instance that later receives the field. */
#[test]
fn computed_field_keys_use_outer_this_and_convert_to_property_key_once() {
    let source = "let conversions=0;const key={toString(){conversions++;return 'captured'}};function create(){return class{[this.key]=7;static [this.key]=9}}const C=create.call({key});const a=new C,b=new C;const r=[conversions,a.captured,b.captured,C.captured].join();";
    assert_eq!(common::eval(source, "r"), "2,7,7,9");
}

/** Private accesses in computed keys check the actual outer receiver before any construction happens. */
#[test]
fn computed_private_field_keys_check_brands_during_class_definition() {
    let source = "const log=[];for(const text of ['class C{[this.#m]=1;#m(){}}','class C{[this.#m]=1;get #m(){}}','class C{[this.#m]=1;set #m(value){}}']){try{Function(text)();log.push(false)}catch(e){log.push(e instanceof TypeError)}}const r=log.join();";
    assert_eq!(common::eval(source, "r"), "true,true,true");
}

/** A class name remains uninitialized throughout computed-key evaluation. */
#[test]
fn computed_field_keys_observe_the_class_name_dead_zone() {
    let source = "let r=false;try{class C{[C]=1}}catch(e){r=e instanceof ReferenceError}";
    assert_eq!(common::eval(source, "r"), "true");
}

/** Classes evaluated at one source site retain distinct captured keys across public JSON transport. */
#[test]
fn computed_key_environments_survive_transport_without_re_evaluation() {
    let sources=vec!["let evaluations=0;function make(key){return class{[(evaluations++,key)]=key}}const A=make('a'),B=make('b');document.getElementById('go').addEventListener('click',()=>console.log(new A().a,new B().b,evaluations));".to_string()];
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
    assert_eq!(common::console(&clicked), ["a b 2"]);
}

/** Ordinary and private field bindings give anonymous function and class values their specified names. */
#[test]
fn field_initializers_infer_public_static_and_private_names() {
    let source = "class C{method=function(){};arrow=()=>{};nested=class{};#private=()=>{};static factory=function(){};read(){return this.#private.name}}const value=new C;const r=[value.method.name,value.arrow.name,value.nested.name,C.factory.name,value.read()].join();";
    assert_eq!(
        common::eval(source, "r"),
        "method,arrow,nested,factory,#private"
    );
}

/** Computed field names are captured before class static effects and apply only to anonymous definitions. */
#[test]
fn computed_fields_name_anonymous_definitions_before_static_effects() {
    let source = "const symbol=Symbol('named');const existing=()=>{};const seen=[];class C{['method']=function(){};[symbol]=()=>{};['nested']=class{static{seen.push(this.name)}};['custom']=class{static name='authored'};['reference']=existing;static ['factory']=()=>{}}const value=new C;const r=[value.method.name,value[symbol].name,value.nested.name,value.custom.name,value.reference===existing,value.reference.name,C.factory.name,seen.join()].join();";
    assert_eq!(
        common::eval(source, "r"),
        "method,[named],nested,authored,true,existing,factory,nested"
    );
}

/** Static field super references use the constructor as home and keep that home through nested arrows. */
#[test]
fn static_initializers_and_arrows_have_the_constructor_home() {
    let source = "class Base{static value=1;static read=()=>this.value}class Derived extends Base{static direct=super.value;static arrow=()=>super.value;static called=()=>super.read();static dynamic=eval('super.value')}const r=[Derived.direct,Derived.arrow(),Derived.called(),Derived.dynamic].join();";
    assert_eq!(common::eval(source, "r"), "1,1,1,1");
}

/** Object literal named evaluation also precedes anonymous class static initialization. */
#[test]
fn object_literal_class_names_exist_before_static_fields() {
    let source = "const object={plain:class{static observed=this.name},['computed']:class{static observed=this.name;static name='custom'}};const r=[object.plain.observed,object.computed.observed,object.computed.name].join();";
    assert_eq!(common::eval(source, "r"), "plain,computed,custom");
}

/** BindThisValue precedes derived fields, so arrows captured before super see the initialized receiver during fields. */
#[test]
fn derived_fields_observe_the_already_bound_lexical_receiver() {
    let source = "let read,fromField,fromBody;class C extends class{}{value=(fromField=read(),7);constructor(){read=()=>this;super();fromBody=this}}const object=new C;const r=[object.value,fromField===object,fromBody===object].join();";
    assert_eq!(common::eval(source, "r"), "7,true,true");
}
