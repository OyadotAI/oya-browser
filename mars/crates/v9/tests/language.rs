/** ECMAScript language and library behavior through the public page API. */
use crate::common;
use common::eval;

/** Classes, inheritance, super calls, getters and statics. */
#[test]
fn classes_inherit_and_call_super() {
    let source = "class A{constructor(x){this.x=x}get twice(){return this.x*2}static make(){return new A(3)}m(){return 'A'}}\
        class B extends A{constructor(){super(5)}m(){return super.m()+'B'}}\
        const r=[new B().twice, new B().m(), A.make().x, new B() instanceof A].join();";
    assert_eq!(eval(source, "r"), "10,AB,3,true");
}

/** Calling a class without new is a TypeError. */
#[test]
fn class_constructors_require_new() {
    let source = "class A{} let r; try{A()}catch(e){r=e instanceof TypeError}";
    assert_eq!(eval(&format!("{source};const out=r;"), "out"), "true");
}

/** Generators suspend and resume with values, and delegate with yield*. */
#[test]
fn generators_resume_and_delegate() {
    let source = "function* g(){const x=yield 1;yield x*2;return 9}const it=g();\
        const r=[it.next().value,it.next(21).value,it.next().done,[...(function*(){yield* [1,2];yield 3})()].join('|')].join();";
    assert_eq!(eval(source, "r"), "1,42,true,1|2|3");
}

/** Returning from a generator runs its finally block. */
#[test]
fn generator_return_runs_finally() {
    let source = "let log=[];function* g(){try{yield 1}finally{log.push('f')}}const it=g();it.next();it.return(5);const r=log.join();";
    assert_eq!(eval(source, "r"), "f");
}

/** Promise jobs, queueMicrotask and await run in specification order after the script. */
#[test]
fn microtasks_and_await_keep_order() {
    let source = "var order=[];Promise.resolve(1).then(v=>order.push('then'+v));queueMicrotask(()=>order.push('micro'));\
        (async()=>{const v=await Promise.resolve(5);order.push('await'+v);console.log(order.join())})();order.push('sync');";
    assert_eq!(
        common::console(&common::load(source)),
        vec!["sync,then1,micro,await5"]
    );
}

/** Promise combinators settle with every result. */
#[test]
fn promise_all_and_all_settled_collect_results() {
    let source = "Promise.all([1,Promise.resolve(2),new Promise(r=>r(3))]).then(v=>console.log(v.join()));\
        Promise.allSettled([Promise.reject(1),2]).then(v=>console.log(v.map(x=>x.status).join()));";
    assert_eq!(
        common::console(&common::load(source)),
        vec!["1,2,3", "rejected,fulfilled"]
    );
}

/** Destructuring with defaults, holes and rest elements. */
#[test]
fn destructuring_binds_defaults_and_rest() {
    let source = "const {a,b:bb=9,...rest}={a:1,c:4,d:5};const [p,,q=7,...more]=[1,2,undefined,4,5];const r=[a,bb,rest.c+rest.d,p,q,more.length].join();";
    assert_eq!(eval(source, "r"), "1,9,9,1,7,2");
}

/** try/catch/finally, labeled loops, switch fallthrough and per-iteration let bindings. */
#[test]
fn control_flow_matches_javascript() {
    let source = "let log=[];function f(){try{return 1}finally{log.push('fin')}}f();\
        outer:for(let i=0;i<3;i++){for(let j=0;j<3;j++){if(j==1)continue outer;if(i==2)break outer;log.push(''+i+j)}}\
        switch(3){case 1:log.push('x');case 3:log.push('three');case 4:log.push('four');break;default:log.push('d')}\
        const fns=[];for(let i=0;i<3;i++)fns.push(()=>i);const r=log.join()+'|'+fns.map(f=>f()).join();";
    assert_eq!(eval(source, "r"), "fin,00,10,three,four|0,1,2");
}

/** typeof and loose equality edge cases. */
#[test]
fn typeof_and_equality_edge_cases() {
    let source = "const r=[typeof null,typeof undeclaredName,typeof function(){},typeof Symbol(),null==undefined,'1'==1,NaN==NaN,[]+[],[1]==1,typeof class{}].join();";
    assert_eq!(
        eval(source, "r"),
        "object,undefined,function,symbol,true,true,false,,true,function"
    );
}

/** Error subclasses carry names, messages and stack text. */
#[test]
fn error_types_have_names_and_messages() {
    let source = "let r=[];for(const E of [TypeError,RangeError,SyntaxError,ReferenceError]){try{throw new E('m')}catch(e){r.push(e.name+':'+e.message+':'+(e instanceof Error))}}\
        try{null.x}catch(e){r.push(e.constructor.name)}const out=r.join()+'|'+(typeof new Error('x').stack);";
    assert_eq!(
        eval(source, "out"),
        "TypeError:m:true,RangeError:m:true,SyntaxError:m:true,ReferenceError:m:true,TypeError|string"
    );
}

/** Getters and setters through literals and defineProperty. */
#[test]
fn accessors_run_through_literals_and_define_property() {
    let source = "let seen;const o={get v(){return 4},set v(x){seen=x}};o.v=7;Object.defineProperty(o,'w',{get(){return this.v+1},enumerable:false});\
        const r=[o.v,seen,o.w,Object.keys(o).join('')].join();";
    assert_eq!(eval(source, "r"), "4,7,5,v");
}

/** Object statics. */
#[test]
fn object_statics_reflect_and_freeze() {
    let source = "const o=Object.create({inherited:1},{own:{value:2,enumerable:true}});const f=Object.freeze({a:1});f.a=2;\
        const r=[Object.keys(o).join(),o.inherited,Object.entries({x:1}).join(),Object.assign({},{a:1},{b:2}).b,Object.isFrozen(f),f.a,Object.getPrototypeOf([])===Array.prototype,JSON.stringify(Object.fromEntries([['k',1]]))].join('|');";
    assert_eq!(eval(source, "r"), "own|1|x,1|2|true|1|true|{\"k\":1}");
}

/** Array methods through ES2023. */
#[test]
fn array_methods_cover_es2023() {
    let source = "const a=[3,1,2];const r=[a.toSorted().join(),a.join(),a.with(0,9).join(),a.at(-1),a.findLast(x=>x<3),[1,[2,[3]]].flat(Infinity).join(),\
        [1,2,3].reduce((s,x)=>s+x,0),[NaN].includes(NaN),Array.from({length:3},(_,i)=>i*2).join(),[1,2,3].splice(1,1).join(),[5,1,10].sort((x,y)=>x-y).join()].join('|');";
    assert_eq!(
        eval(source, "r"),
        "1,2,3|3,1,2|9,1,2|2|2|1,2,3|6|true|0,2,4|2|1,5,10"
    );
}

/** String methods. */
#[test]
fn string_methods_cover_common_use() {
    let source = "const r=[' Abc '.trim().toLowerCase().padStart(5,'*'),'a-b-c'.split('-').length,'abc'.at(-1),'x'.repeat(3),'aXbX'.replaceAll('X','.'),\
        'hello'.startsWith('he'),'%41'.length,encodeURIComponent('a b&c'),decodeURIComponent('%E2%82%AC'),'😀'.codePointAt(0)].join('|');";
    assert_eq!(
        eval(source, "r"),
        "**abc|3|c|xxx|a.b.|true|3|a%20b%26c|€|128512"
    );
}

/** Regular expressions: exec, groups, global replace with functions, split, sticky and lookarounds. */
#[test]
fn regular_expressions_run_on_the_owned_engine() {
    let source = "const m='2024-10-23'.match(/(?<y>\\d{4})-(?<m>\\d\\d)/);const re=/a/y;re.lastIndex=1;\
        const r=[m.groups.y,m.index,'a1b22'.replace(/\\d+/g,d=>'<'+d+'>'),'a, b,c'.split(/,\\s*/).join('|'),re.test('ba'),/(?<=\\$)\\d+/.exec('cost $42')[0],\
        /^[\\w.]+@\\w+\\.com$/i.test('Me.X@Site.COM'),'aaa'.match(/a/g).length,[...'a1b2'.matchAll(/\\d/g)].map(x=>x[0]).join('')].join(';');";
    assert_eq!(
        eval(source, "r"),
        "2024;0;a<1>b<22>;a|b|c;true;42;true;3;12"
    );
}

/** Catastrophic backtracking stops at the step bound instead of hanging, as a RangeError. */
#[test]
fn regular_expression_steps_are_bounded() {
    let execution = common::load("const r=/^(a+)+$/.test('a'.repeat(40)+'b');");
    let message = common::failure(&execution).expect("bounded failure");
    assert!(
        message.contains("RangeError: Maximum regular expression steps"),
        "{message}"
    );
    // Catchable, so one slow pattern never stops the page's other scripts.
    let caught = common::load("let r;try{/^(a+)+$/.test('a'.repeat(40)+'b')}catch(e){r=e.name}");
    assert_eq!(common::global(&caught, "r"), "RangeError");
}

/** A plain search over text longer than the fixed step bound finishes: the bound grows with the
input, as YouTube's scans of its megabyte page data need. */
#[test]
fn long_text_searches_are_not_cut_off() {
    let execution = common::load(
        "const t='x'.repeat(1000000);const r=[t.search(/y/),/^x/.test(t),t.replace(/q/g,'')===t].join();",
    );
    assert_eq!(common::failure(&execution), None);
    assert_eq!(common::global(&execution, "r"), "-1,true,true");
}

/** A greedy loop over a single atom runs flat over a million characters, while a loop whose
body carries a capture still recurses per repetition and fails as a bounded limit instead of
overflowing the native stack. */
#[test]
fn deep_matches_are_bounded_not_a_crash() {
    let execution = common::load("const r=String(/x+$/.test('x'.repeat(1000000)));");
    assert_eq!(common::failure(&execution), None);
    assert_eq!(common::global(&execution, "r"), "true");
    let execution = common::load("const r=/(x)+$/.test('x'.repeat(1000000));");
    let message = common::failure(&execution).expect("bounded failure");
    assert!(
        message.contains("RangeError: Maximum regular expression steps"),
        "{message}"
    );
}

/** Map, Set, WeakMap and iteration order. */
#[test]
fn collections_keep_insertion_order() {
    let source = "const m=new Map([[1,'a']]);m.set('k',2);m.delete(1);m.set(1,'b');const s=new Set([3,1,3,2]);const w=new WeakMap();const key={};w.set(key,5);\
        const r=[[...m.keys()].join(),m.size,[...s].join(),w.get(key),w.has({})].join('|');";
    assert_eq!(eval(source, "r"), "k,1|2|3,1,2|5|false");
}

/** JSON round trips with replacers, revivers and indentation. */
#[test]
fn json_parse_and_stringify() {
    let source = "const r=[JSON.stringify({a:[1,{b:null}],u:undefined,d:new Date(0)}),JSON.stringify([1,2],null,1).length,JSON.parse('{\"x\":[1,2]}',(k,v)=>k==='x'?v.length:v).x,\
        JSON.stringify({a:1,b:2},['b'])].join('|');";
    assert_eq!(
        eval(source, "r"),
        "{\"a\":[1,{\"b\":null}],\"d\":\"1970-01-01T00:00:00.000Z\"}|10|2|{\"b\":2}"
    );
}

/** Date uses the host clock and timezone offset deterministically. */
#[test]
fn date_reads_the_virtual_clock() {
    let source = "const d=new Date(Date.UTC(2020,0,15,10,30));const r=[d.toISOString(),Date.now(),d.getUTCDay(),new Date('2020-02-03').getTime(),typeof Date()].join('|');";
    assert_eq!(
        eval(source, "r"),
        "2020-01-15T10:30:00.000Z|1700000000000|3|1580688000000|string"
    );
}

/** Math.random is seeded and deterministic for one page (crypto randomness is separate). */
#[test]
fn math_random_is_seeded_by_the_host() {
    let mut environment = common::environment();
    environment.random_seed = 42;
    let first = common::load_with("const r=Math.random();", &environment);
    let second = common::load_with("const r=Math.random();", &environment);
    assert_eq!(common::global(&first, "r"), common::global(&second, "r"));
}

/** eval and new Function compile with V9's own parser. */
#[test]
fn dynamic_code_compiles_with_the_owned_parser() {
    let source = "var g=10;const r=[eval('g+1'),new Function('a','b','return a*b+g')(3,4),eval('(function(){return this})()')===globalThis].join();";
    assert_eq!(eval(source, "r"), "11,22,true");
}

/** Optional chaining, nullish coalescing and logical assignment. */
#[test]
fn optional_chaining_and_logical_assignment() {
    let source = "const o={a:{f(){return this===o.a}}};let n=null;n??='d';const r=[o?.a?.f(),o.b?.c,o.b?.(),n,o.missing?.x.y].join();";
    assert_eq!(eval(source, "r"), "true,,,d,");
}

/** Tagged templates receive raw and cooked chunks. */
#[test]
fn tagged_templates_receive_chunks() {
    let source =
        "function tag(s,...v){return s.raw.length+':'+v.join('+')}const r=tag`a${1}b${2}c`;";
    assert_eq!(eval(source, "r"), "3:1+2");
}

/** Deep recursion stops at the frame bound with a catchable RangeError. */
#[test]
fn call_depth_is_bounded_and_catchable() {
    let source =
        "function f(){return f()}let r;try{f()}catch(e){r=e instanceof RangeError}const out=r;";
    assert_eq!(eval(source, "out"), "true");
}

/** Run a probe fixture; it logs `FAIL ...` for every failed check. */
fn probe(source: &str) -> Vec<String> {
    let execution = common::load(source);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    common::console(&execution)
}

/** The broad language probe passes every check. */
#[test]
fn language_probe_passes() {
    assert_eq!(
        probe(include_str!("fixtures/language-probe.js")),
        ["lang done", "async done"]
    );
}

/** The semantics probe (arguments in arrows, private members, number formatting...) passes. */
#[test]
fn semantics_probe_passes() {
    assert_eq!(
        probe(include_str!("fixtures/semantics-probe.js")),
        ["probe2 done"]
    );
}

/** A destructuring assignment evaluates each member target's reference before it reads the
value (GetV or IteratorStep), and converts a computed target key only when it stores. */
#[test]
fn destructuring_targets_evaluate_before_values() {
    let source = "const log=[];const o={set q(v){log.push('set'+v)}};\
        const t=()=>(log.push('t'),o);const k={toString(){log.push('k');return 'q'}};\
        ({a:t().q,b:t()[k]}={get a(){log.push('a');return 1},get b(){log.push('b');return 2}});\
        [t().q]={[Symbol.iterator](){return {next(){log.push('n');return {value:3,done:false}},return(){return {}}}}};\
        const r=log.join();";
    assert_eq!(eval(source, "r"), "t,a,set1,t,b,k,set2,t,n,set3");
}
