/**
 * The baseline JIT must compute what the interpreter computes. The JIT switch is read once per
 * process, so each tier runs in a child process of this same test binary with its environment set;
 * the child prints every program's result, and the parent requires the two outputs to match. The
 * compiled child must also report compilations on stderr, so the comparison cannot pass by the JIT
 * never running.
 */
mod common;

use common::eval;
use std::process::Command;

/** Set to the results file's path in the child, so it runs the programs instead of spawning more. */
const CHILD_ENV: &str = "MARS_V9_JIT_EQUIVALENCE_CHILD";
/** The compilation report the JIT writes to stderr when tracing is on. */
const COMPILED_REPORT: &str = "V9 JIT compiled";

/** Small programs that together touch loops, calls, recursion, closures, exceptions, finally,
generators, native callbacks, labels, do-while and switch. Each leaves its answer in `r`. */
const PROGRAMS: &[(&str, &str)] = &[
    (
        "loop sum",
        "let s=0; for(let i=0;i<5000;i++) s+=i; const r=s;",
    ),
    (
        "calls",
        "function f(x){return x*2+1} let s=0; for(let i=0;i<3000;i++) s+=f(i); const r=s;",
    ),
    (
        "recursion",
        "function fib(n){return n<2?n:fib(n-1)+fib(n-2)} const r=fib(18);",
    ),
    (
        "closures",
        "function mk(){let c=0;return()=>++c} const g=mk(); let s=0; for(let i=0;i<2000;i++) s=g(); const r=s;",
    ),
    (
        "exceptions",
        "function t(i){ if(i%7===0) throw new Error('e'+i); return i } let n=0; for(let i=0;i<2000;i++){ try{ n+=t(i) } catch(e){ n-=1 } } const r=n;",
    ),
    (
        "finally",
        "let log=0; function f(){ try { return 1 } finally { log++ } } for(let i=0;i<1500;i++) f(); const r=log;",
    ),
    (
        "generators",
        "function* g(){ for(let i=0;i<2000;i++) yield i } let s=0; for (const v of g()) s+=v; const r=s;",
    ),
    (
        "strings",
        "let t=''; for(let i=0;i<1000;i++) t+=String.fromCharCode(65+i%26); const r=t.length+t.charCodeAt(5);",
    ),
    (
        "methods and this",
        "const o={n:0, inc(){ this.n++; return this }}; for(let i=0;i<3000;i++) o.inc(); const r=o.n;",
    ),
    (
        "do-while and switch",
        "let i=0,s=0; do { s+=i; i++ } while(i<3000); switch(s%3){case 0: s+=1; break; default: s+=2} const r=s;",
    ),
    (
        "native callbacks",
        "const a=[]; for(let i=0;i<1000;i++) a.push(i); const r=a.map(x=>x*2).filter(x=>x%3===0).reduce((p,c)=>p+c,0);",
    ),
    (
        "labelled loops",
        "let s=0; outer: for(let i=0;i<100;i++){ for(let j=0;j<100;j++){ if(j>i) continue outer; if(i*j>5000) break outer; s+=j } } const r=s;",
    ),
    (
        "error from depth",
        "function d(n){ return n===0 ? (()=>{throw new RangeError('x')})() : d(n-1)} let m=''; try{d(50)}catch(e){m=e.name} const r=m;",
    ),
];

/** The parent runs both tiers and compares them; the child prints results for one tier. */
#[test]
fn jit_matches_interpreter() {
    if let Some(results_path) = std::env::var_os(CHILD_ENV) {
        let lines: Vec<String> = PROGRAMS
            .iter()
            .map(|(name, source)| format!("{name}={}", eval(source, "r")))
            .collect();
        std::fs::write(results_path, lines.join("\n")).expect("write child results");
        return;
    }
    let (interpreted, _) = run_child("interpreter", &[("MARS_V9_JIT", "0")]);
    let (compiled, report) = run_child(
        "compiled",
        &[
            ("MARS_V9_JIT", "1"),
            ("MARS_V9_JIT_THRESHOLD", "0"),
            ("MARS_V9_JIT_TRACE", "1"),
        ],
    );
    assert_eq!(
        compiled.len(),
        PROGRAMS.len(),
        "every program reported a result"
    );
    assert_eq!(
        interpreted, compiled,
        "the compiled tier must match the interpreter"
    );
    assert!(
        report.contains(COMPILED_REPORT),
        "the JIT must compile at least one function in this run:\n{report}"
    );
}

/** Runs this test in a child with `environment`, returning its results and its stderr. The
results travel through a file, because the child's stdout shares a line with the harness. */
fn run_child(tier: &str, environment: &[(&str, &str)]) -> (Vec<String>, String) {
    let executable = std::env::current_exe().expect("test executable");
    let results_path = std::env::temp_dir().join(format!(
        "v9-jit-equivalence-{}-{tier}.txt",
        std::process::id()
    ));
    let output = Command::new(executable)
        .args([
            "--exact",
            "jit_matches_interpreter",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD_ENV, &results_path)
        .envs(environment.iter().copied())
        .output()
        .expect("spawn child");
    assert!(
        output.status.success(),
        "child failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let results = std::fs::read_to_string(&results_path).expect("child results");
    let _ = std::fs::remove_file(&results_path);
    let results = results.lines().map(str::to_string).collect();
    (
        results,
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}
