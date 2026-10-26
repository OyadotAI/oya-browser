/** Direct eval inherits real lexical private environments while indirect eval remains globally scoped. */
mod common;

/** Static and instance private declarations are visible to direct eval in their lexical methods. */
#[test]
fn direct_eval_reads_lexical_static_and_instance_private_elements() {
    let source = "class C{#value=7;static #staticValue=9;read(){return eval('this.#value')}static read(){return eval('this.#staticValue')}}const r=[new C().read(),C.read()].join();";
    assert_eq!(common::eval(source, "r"), "7,9");
}

/** Nearest private declarations shadow outer declarations, including nested eval-created closures. */
#[test]
fn nested_private_eval_uses_the_nearest_lexical_class() {
    let source = "class Outer{#value='outer';make(){const outer=this;return class Inner{#value='inner';read(){return eval('this.#value')}static readOuter(){return eval('outer.#value')}}}}const Inner=new Outer().make();let threw=false;try{Inner.readOuter()}catch(e){threw=e instanceof TypeError}const r=[new Inner().read(),threw].join();";
    assert_eq!(common::eval(source, "r"), "inner,true");
}

/** Eval-created readers retain per-evaluation brands rather than just the source spelling of a name. */
#[test]
fn eval_private_readers_keep_distinct_class_evaluation_brands() {
    let source = "function make(){return class{#value=3;static reader(){return eval('object=>object.#value')}}}const A=make(),B=make(),readA=A.reader(),readB=B.reader();let threw=false;try{readA(new B())}catch(e){threw=e instanceof TypeError}const r=[readA(new A()),readB(new B()),threw].join();";
    assert_eq!(common::eval(source, "r"), "3,3,true");
}

/** Missing and indirectly evaluated private references fail before preceding source effects can run. */
#[test]
fn unknown_and_indirect_private_references_are_early_errors() {
    let source = "let effects=0;class C{#value=1;test(){const out=[];try{eval('effects++;this.#missing')}catch(e){out.push(e instanceof SyntaxError)}try{(0,eval)('effects++;this.#value')}catch(e){out.push(e instanceof SyntaxError)}return out}}const r=[...new C().test(),effects].join();";
    assert_eq!(common::eval(source, "r"), "true,true,0");
}

/** Persisted eval code keeps its lexical private ownership after recompilation and JSON transport. */
#[test]
fn eval_private_closures_survive_public_state_transport() {
    let sources=vec!["class C{#value=11;static reader(){return eval('object=>object.#value')}}const object=new C,reader=C.reader();document.getElementById('go').addEventListener('click',()=>console.log(reader(object)));".to_string()];
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
    assert_eq!(common::console(&clicked), ["11"]);
}

/** A nested class's heritage captures the outer private environment before inner declarations shadow it. */
#[test]
fn heritage_closures_capture_only_the_outer_private_environment() {
    let source = "let factory;class Outer{#value='outer';static Inner=class Inner extends(factory=function(){return class Reader{read(object){return object.#value}readEval(object){return eval('object.#value')}}}){#value='inner'}}const outer=new Outer,inner=new Outer.Inner,Reader=factory(),reader=new Reader;let rejected=false;try{reader.read(inner)}catch(e){rejected=e instanceof TypeError}const r=[reader.read(outer),reader.readEval(outer),rejected].join();";
    assert_eq!(common::eval(source, "r"), "outer,outer,true");
}

/** Claimed lexical private scopes must exist on the caller's actual environment chain. */
#[test]
fn missing_actual_private_environments_are_rejected_before_eval_execution() {
    let sources=vec!["let effects=0;class C{#value=1;read(){return eval('effects++;this.#value')}}const object=new C;document.getElementById('go').addEventListener('click',()=>console.log(object.read()));".to_string()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let mut wire = serde_json::to_value(loaded.state).unwrap();
    let environments = wire["heap"][1].as_array_mut().unwrap();
    let mut removed = false;
    for record in environments {
        removed |= record[1].as_object_mut().unwrap().remove("c").is_some();
    }
    assert!(removed);
    let state = serde_json::from_value(wire).unwrap();
    let result = mars_v9::click_page(common::input(&sources, &host, &environment), state, 3);
    assert_eq!(result.unwrap_err().code, "invalid_request");
}

/** Direct eval inherits a derived constructor's super-call permission and binds the real constructor receiver. */
#[test]
fn direct_eval_super_initializes_the_derived_receiver_once() {
    let source = "const log=[];class Base{constructor(){log.push('base')}}class Derived extends Base{value=(log.push('field'),7);constructor(){eval('super()');log.push(eval('this.value'))}}const object=new Derived;const r=[object.value,log.join(':')].join();";
    assert_eq!(common::eval(source, "r"), "7,base:field:7");
}

/** Eval inherits lexical home and new.target rules without admitting return or super in unrelated functions. */
#[test]
fn eval_receiver_grammar_does_not_leak_into_unrelated_functions() {
    let source = "class Base{method(){return 3}}class Derived extends Base{method(){const arrow=()=>eval('super.method()');return arrow()}}function target(){return eval('new.target')}let refused=0;for(const text of ['return 1','super.x','super()']){try{eval(text)}catch(e){if(e instanceof SyntaxError)refused++}}const r=[new Derived().method(),new target()===target,refused].join();";
    assert_eq!(common::eval(source, "r"), "3,true,3");
}

/** An eval in a constructor's lexical arrow initializes the same receiver environment as a direct super call. */
#[test]
fn eval_super_in_a_constructor_arrow_initializes_the_shared_receiver() {
    let source = "class C extends class{}{value=1;constructor(){const initialize=()=>{eval('super()')};initialize()}}const r=new C().value;";
    assert_eq!(common::eval(source, "r"), "1");
}

/** Nested derived constructors each bind their own receiver before an enclosing constructor's super call. */
#[test]
fn nested_constructor_arrows_bind_their_own_super_receiver() {
    let source = "class X extends class{}{value=1;constructor(){const initialize=()=>{class P extends class{}{constructor(){const initialize=()=>{super()};initialize()}}const p=new P;if(p.value!==undefined)throw 'wrong';super()};initialize()}}const r=new X().value;";
    assert_eq!(common::eval(source, "r"), "1");
}
