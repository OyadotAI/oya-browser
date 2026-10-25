/** Async generators, for await and AsyncFromSyncIterator through the public page API. */
mod common;

/** The console lines a script logs once its microtasks drain. */
fn logged(source: &str) -> Vec<String> {
    let execution = common::load(source);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    common::console(&execution)
}

/** next() returns promises of iterator results; awaits and yields interleave in order. */
#[test]
fn async_generator_yields_through_promises() {
    let source = "async function* g(){const x=yield 1;await null;yield x*2;return 'end'}\
        const it=g();const out=[];\
        it.next().then(r=>out.push(r.value+':'+r.done));\
        it.next(21).then(r=>out.push(r.value+':'+r.done));\
        it.next().then(r=>out.push(r.value+':'+r.done));\
        it.next().then(r=>{out.push(r.value+':'+r.done);console.log(out.join())});";
    assert_eq!(
        logged(source),
        vec!["1:false,42:false,end:true,undefined:true"]
    );
}

/** A yielded promise is awaited, and a rejected one throws into the body. */
#[test]
fn async_generator_awaits_yielded_promises() {
    let source = "async function* g(){try{yield Promise.reject('no')}catch(e){yield 'caught '+e}}\
        const it=g();it.next().then(r=>console.log(r.value));";
    assert_eq!(logged(source), vec!["caught no"]);
}

/** return() on a suspended generator runs finally; on a fresh one it awaits the value. */
#[test]
fn async_generator_return_runs_finally_and_awaits() {
    let source = "const log=[];async function* g(){try{yield 1}finally{log.push('f')}}\
        const a=g();a.next().then(()=>a.return(5)).then(r=>{log.push(r.value+':'+r.done);\
        return g().return(Promise.resolve(7))}).then(r=>{log.push(r.value);console.log(log.join())});";
    assert_eq!(logged(source), vec!["f,5:true,7"]);
}

/** throw() before the first next() completes the generator and rejects. */
#[test]
fn async_generator_throw_at_start_rejects() {
    let source = "async function* g(){yield 1}const it=g();\
        it.throw(new Error('x')).catch(e=>it.next()).then(r=>console.log(String(r.done)));";
    assert_eq!(logged(source), vec!["true"]);
}

/** for await iterates async generators, sync iterables (awaiting values) and breaks close. */
#[test]
fn for_await_iterates_and_closes() {
    let source = "async function* g(){try{yield 1;yield 2;yield 3}finally{console.log('closed')}}\
        (async()=>{const out=[];for await(const x of g()){out.push(x);if(x==2)break}\
        for await(const y of [Promise.resolve('a'),'b'])out.push(y);console.log(out.join())})();";
    assert_eq!(logged(source), vec!["closed", "1,2,a,b"]);
}

/** yield* in an async generator delegates to async and sync iterables. */
#[test]
fn async_yield_star_delegates() {
    let source = "async function* inner(){yield 'i1';return 'r'}\
        async function* g(){const r=yield* inner();yield r;yield* ['s1','s2']}\
        (async()=>{const out=[];for await(const x of g())out.push(x);console.log(out.join())})();";
    assert_eq!(logged(source), vec!["i1,r,s1,s2"]);
}

/** The intrinsics are wired: prototypes, toStringTag and the hidden constructor. */
#[test]
fn async_generator_intrinsics() {
    let source = "async function* g(){}const AGF=Object.getPrototypeOf(g);\
        const f=new AGF.constructor('a','yield a');\
        const r=[Object.prototype.toString.call(g()),AGF[Symbol.toStringTag],\
        Object.getPrototypeOf(g.prototype)===AGF.prototype,typeof g()[Symbol.asyncIterator],\
        Object.getPrototypeOf(f)===AGF].join();";
    assert_eq!(
        common::eval(source, "r"),
        "[object AsyncGenerator],AsyncGeneratorFunction,true,function,true"
    );
}

/** The early errors of async generator heads: no await or yield in formals, escaped keywords
are names, and `for await (async of ...)` is allowed. */
#[test]
fn async_generator_early_errors() {
    let source = "const errs=['(async function*(x=await 1){})','(async function*(x=yield){})',\
        '\\\\u0061sync function* f(){}','async function* f(){for await(var x o\\\\u0066 []);}']\
        .map(name);function name(s){try{(0,eval)(s);return 'ok'}catch(e){return e.name}}\
        errs.push(name('async function* f(){for await(var x of [])let\\n{}}'));\
        let async;(async()=>{for await(async of [7]);errs.push(async);console.log(errs.join())})();";
    assert_eq!(
        logged(source),
        vec!["SyntaxError,SyntaxError,SyntaxError,SyntaxError,ok,7"]
    );
}

/** Non-simple parameters initialize in order (TDZ), eval cannot redeclare them, and an object
pattern reads each getter once while null still throws. */
#[test]
fn parameters_initialize_in_order() {
    let source = "const name=f=>{try{f();return 'none'}catch(e){return e.name}};\
        let c=0;const o={get v(){c++;return 2}};(async function*({...x}){})(o).next();\
        const r=[name(()=>(async function*(x=y,y){})().next()),\
        name(()=>(function(x=y,y){})()),name(()=>(function(a=eval('var a=1')){})()),\
        name(()=>{var {}=null}),c,(function(a,b=a+1,...r){a=a+10;return [a,b,r.length].join('/')})(2,5,6)].join();";
    assert_eq!(
        common::eval(source, "r"),
        "ReferenceError,ReferenceError,SyntaxError,TypeError,1,12/5/1"
    );
}

/** The hidden constructors' `name` and `length` are configurable: delete removes them. */
#[test]
fn hidden_constructor_length_deletes() {
    let source = "const G=Object.getPrototypeOf(async function*(){}).constructor;\
        const r=[delete G.length,G.hasOwnProperty('length'),delete G.name,G.hasOwnProperty('name')].join();";
    assert_eq!(common::eval(source, "r"), "true,false,true,false");
}
