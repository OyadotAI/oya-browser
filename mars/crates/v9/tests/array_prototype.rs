/** Array.prototype is an array before materialization and retains exotic semantics afterward. */
mod common;

/** The initial intrinsic length is writable but cannot enumerate or disappear. */
#[test]
fn array_prototype_has_an_array_length_descriptor() {
    let source = "const p=Array.prototype;const d=Object.getOwnPropertyDescriptor(p,'length');\
        let r=[Array.isArray(p),p.length,d.value,d.writable,d.enumerable,d.configurable,\
        Object.getOwnPropertyNames(p).filter(x=>x==='length').length].join();";
    assert_eq!(common::eval(source, "r"), "true,0,0,true,false,false,1");
}

/** Adding and truncating prototype indices must update length through ordinary array rules. */
#[test]
fn array_prototype_mutations_keep_array_exotic_semantics() {
    let source = "const p=Array.prototype;p[3]='x';const before=[p.length,p[3],[,,,][3]];\
        p.length=1;let r=before.concat([p.length,p.hasOwnProperty('3'),Array.isArray(p)]).join();";
    assert_eq!(common::eval(source, "r"), "4,x,x,1,false,true");
}

/** Materialized prototype arrays survive the same validated state boundary as ordinary arrays. */
#[test]
fn materialized_array_prototype_survives_state_validation() {
    let sources = ["Array.prototype[1]=7;const before=Array.prototype.length;".to_string()];
    let host = common::host();
    let environment = common::environment();
    let execution = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    let state = common::round_trip(execution.state);
    let execution =
        mars_v9::run_timers(common::input(&sources, &host, &environment), state, 0.0, 1).unwrap();
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "before"), "2");
}

/** Array classification happens before an authored tag can replace the default tag. */
#[test]
fn object_to_string_recognizes_prototype_and_proxy_arrays() {
    let source = "const proxy=new Proxy([],{});let r=Object.prototype.toString.call(Array.prototype)+','+\
        Object.prototype.toString.call(proxy);proxy[Symbol.toStringTag]='Custom';\
        r+=','+Object.prototype.toString.call(proxy);";
    assert_eq!(
        common::eval(source, "r"),
        "[object Array],[object Array],[object Custom]"
    );
}

/** Push performs ordinary Set so inherited setters see a new element instead of being bypassed. */
#[test]
fn push_observes_inherited_index_setters() {
    let source = "let seen=0;Object.defineProperty(Array.prototype,'0',{set(value){seen=value},configurable:true});\
        const a=[];const length=a.push(7);let r=[length,a.hasOwnProperty('0'),seen,a.length].join();\
        delete Array.prototype[0];";
    assert_eq!(common::eval(source, "r"), "1,false,7,1");
}

/** Even a push with no arguments must attempt its final length store and throw if read-only. */
#[test]
fn push_without_arguments_observes_readonly_length() {
    let source = "const a=[1];Object.defineProperty(a,'length',{writable:false});let r='returned';\
        try{a.push()}catch(e){r=e.name}r+=','+a.length;";
    assert_eq!(common::eval(source, "r"), "TypeError,1");
}

/** Generic receivers may have safe-integer lengths larger than any real array's length. */
#[test]
fn push_rejects_growth_past_the_maximum_safe_integer_before_writing() {
    let source = "const a={length:9007199254740991};let r='returned';\
        try{Array.prototype.push.call(a,7)}catch(e){r=e.name}\
        r+=','+a.hasOwnProperty('9007199254740991');";
    assert_eq!(common::eval(source, "r"), "TypeError,false");
}
