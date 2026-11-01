/** Interpreter speed: hermetic checks that the fast paths keep behaviour, plus an ignored
micro-benchmark (`cargo test --release -p mars-v9 --test perf -- --ignored --nocapture bench`). */
use crate::common;
use common::eval;
use std::time::Instant;

/** Each workload: a name and a script that leaves its checksum in `r`. */
const BENCH: &[(&str, &str)] = &[
    (
        "prop get/set",
        "const o={a:1,b:2,c:3};for(let i=0;i<300000;i++){o.a=o.b+o.c;o.b=o.a-1}const r=o.a+o.b;",
    ),
    (
        "method calls",
        "const o={n:0,inc(){this.n++}};for(let i=0;i<200000;i++)o.inc();const r=o.n;",
    ),
    (
        "closures",
        "function mk(){let c=0;return()=>++c}const f=mk();let s=0;for(let i=0;i<200000;i++)s=f();const r=s;",
    ),
    (
        "array push/index",
        "const a=[];for(let i=0;i<200000;i++)a.push(i);let s=0;for(let i=0;i<a.length;i++)s+=a[i];const r=s;",
    ),
    (
        "string concat",
        "let t='';for(let i=0;i<50000;i++)t+='x';const r=t.length;",
    ),
    (
        "object literals",
        "let s=0;for(let i=0;i<100000;i++){const p={x:i,y:i+1,z:2};s+=p.x+p.y+p.z}const r=s;",
    ),
    (
        "prototype get",
        "class P{constructor(){this.v=1}get(){return this.v}}const p=new P();let s=0;for(let i=0;i<200000;i++)s+=p.get();const r=s;",
    ),
];

/** Time every workload through the public page API; ignored because it measures, not checks. */
#[test]
#[ignore]
fn bench() {
    let only = std::env::var("BENCH_ONLY").unwrap_or_default();
    let repeat: usize = std::env::var("BENCH_REPEAT")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(1);
    for (name, source) in BENCH.iter().cycle().take(BENCH.len() * repeat) {
        if !name.contains(only.as_str()) {
            continue;
        }
        let start = Instant::now();
        let result = eval(source, "r");
        println!(
            "{name:18} {:>8.1} ms  r={result}",
            start.elapsed().as_secs_f64() * 1000.0
        );
    }
}

/** Every workload computes the same answer the language requires. */
#[test]
fn workloads_compute_their_answers() {
    let small: Vec<(String, &str)> = BENCH
        .iter()
        .map(|(_, source)| {
            (
                source
                    .replace("300000", "30")
                    .replace("200000", "20")
                    .replace("100000", "10")
                    .replace("50000", "5"),
                "",
            )
        })
        .collect();
    let expected = ["125", "20", "20", "190", "5", "120", "20"];
    for ((source, _), want) in small.iter().zip(expected) {
        assert_eq!(eval(source, "r"), want, "{source}");
    }
}

/** A cached load still sees a property added later, deleted, or shadowed on the instance. */
#[test]
fn cached_loads_follow_shape_changes() {
    let source = "function get(o){return o.k}const proto={k:'p'};const o=Object.create(proto);\
        const seen=[];for(let i=0;i<3;i++)seen.push(get(o));o.k='own';seen.push(get(o));\
        delete o.k;seen.push(get(o));proto.k='q';seen.push(get(o));\
        Object.defineProperty(o,'k',{get(){return 'acc'},configurable:true});seen.push(get(o));\
        const r=seen.join();";
    assert_eq!(eval(source, "r"), "p,p,p,own,p,q,acc");
}

/** A cached store respects a non-writable property, a setter and a frozen object. */
#[test]
fn cached_stores_respect_attributes() {
    let source = "'use strict';function put(o,v){o.k=v}const a={k:1};put(a,2);put(a,3);\
        const log=[a.k];Object.defineProperty(a,'k',{writable:false});\
        try{put(a,4)}catch(e){log.push(e instanceof TypeError)}log.push(a.k);\
        const b={set k(v){log.push('set'+v)}};put(b,5);\
        const c=Object.freeze({k:1});try{put(c,6)}catch(e){log.push('frozen')}\
        const r=log.join();";
    assert_eq!(eval(source, "r"), "3,true,3,set5,frozen");
}

/** One site seeing many shapes still reads each object's own value. */
#[test]
fn polymorphic_sites_read_each_shape() {
    let source = "function get(o){return o.v}const objs=[{v:1},{a:0,v:2},{b:0,c:0,v:3},{v:4,z:0}];\
        let s=[];for(let n=0;n<3;n++)for(const o of objs)s.push(get(o));const r=s.join('');";
    assert_eq!(eval(source, "r"), "123412341234");
}

/** A cached site stays exact when objects leave their shape: a key deleted from the middle
(dictionary mode), many keys (dictionary mode), and a data property redefined as an accessor. */
#[test]
fn cached_sites_follow_dictionary_objects_and_redefinitions() {
    let source = "function get(o){return o.v}function put(o,x){o.v=x}const a={u:0,v:1,w:2};\
        const seen=[];for(let i=0;i<3;i++)seen.push(get(a));delete a.u;seen.push(get(a));\
        put(a,5);seen.push(a.v);const big={};for(let i=0;i<40;i++)big['k'+i]=i;big.v=7;\
        seen.push(get(big));put(big,8);seen.push(big.v);const b={u:0,v:1,w:2};get(b);put(b,3);\
        Object.defineProperty(b,'v',{get(){return 'g'},set(x){seen.push('s'+x)}});\
        seen.push(get(b));put(b,9);const r=seen.join();";
    assert_eq!(eval(source, "r"), "1,1,1,1,5,7,8,g,s9");
}

/** A read of a lexical global before its declaration still throws, now that the found-binding
path builds its message only on that error. */
#[test]
fn global_reads_keep_their_dead_zone() {
    let source = "let r;try{x}catch(e){r=e instanceof ReferenceError}let x=1;";
    assert_eq!(eval(source, "r"), "true");
}

/** Time WPT's reflection-text page scripts (testharness.js, the reflection harnesses and
reflection.js) in one V9 request, the workload that hit the script deadline. Ignored: it reads
the upstream checkout at ~/Dev/upstream/wpt and measures rather than checks. */
#[test]
#[ignore]
fn bench_reflection() {
    let root = std::path::PathBuf::from(std::env::var("HOME").unwrap()).join("Dev/upstream/wpt");
    let page = std::env::var("BENCH_PAGE").unwrap_or_else(|_| "text".into());
    let files = [
        "resources/testharness.js".to_string(),
        "html/dom/original-harness.js".into(),
        "html/dom/new-harness.js".into(),
        format!("html/dom/elements-{page}.js"),
        "html/dom/reflection.js".into(),
    ];
    let Ok(sources) = files
        .iter()
        .map(|file| std::fs::read_to_string(root.join(file)))
        .collect::<Result<Vec<_>, _>>()
    else {
        return;
    };
    let host = common::host();
    let environment = common::environment();
    let start = Instant::now();
    let execution =
        mars_v9::load_page(common::input(&sources, &host, &environment)).expect("page loads");
    println!(
        "reflection-{page}: {:.1} ms, failures {:?}",
        start.elapsed().as_secs_f64() * 1000.0,
        execution
            .failed
            .iter()
            .map(|failure| failure.error.message.clone())
            .collect::<Vec<_>>()
    );
}
