/**
 * The C3 yardstick (mars/docs/C3-value-representation.md): nanoseconds per iteration of six hot
 * operations, each a million-iteration loop inside a function, best of three. Ignored in normal
 * runs; run it before and after an engine change with
 * `cargo test --release --offline -p mars-v9 --test value_bench -- --ignored --nocapture`.
 */
mod common;

/** Iterations per row. */
const ITERATIONS: u32 = 1_000_000;
/** Runs per row; the best counts, so a busy machine inflates fewer rows. */
const RUNS: usize = 3;

/** One row: setup code, then the loop body over `i`. */
const ROWS: [(&str, &str, &str); 7] = [
    ("empty loop", "", ""),
    ("arithmetic", "let a=1,b=2,x=0;", "x=a+b;"),
    ("call", "function g(v){return v}let x=0;", "x=g(i);"),
    ("property read, own", "const o={p:1};let x=0;", "x=o.p;"),
    (
        "property read, prototype method",
        "class C{m(){}}const o=new C();let x;",
        "x=o.m;",
    ),
    ("new", "class C{}let x;", "x=new C();"),
    ("indexed read", "const a=[1,2,3,4];let x=0;", "x=a[i&3];"),
];

/** Wall time of one row's loop, per iteration, in nanoseconds. */
fn row_ns(setup: &str, body: &str) -> f64 {
    let source = format!(
        "function run(){{{setup}for(let i=0;i<{ITERATIONS};i++){{{body}}}}}run();const done='y';"
    );
    let started = std::time::Instant::now();
    assert_eq!(common::eval(&source, "done"), "y");
    started.elapsed().as_nanos() as f64 / f64::from(ITERATIONS)
}

/** Prints every row's best time; the C3 gate compares these before and after a change. */
#[test]
#[ignore = "benchmark: run by hand in release"]
fn hot_operations_per_iteration() {
    for (name, setup, body) in ROWS {
        let best = (0..RUNS)
            .map(|_| row_ns(setup, body))
            .fold(f64::INFINITY, f64::min);
        eprintln!("BENCH {name:<34} {best:>8.0} ns");
    }
}
