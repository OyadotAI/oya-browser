/**
 * Differential fuzzing of the JIT against the interpreter. A seeded generator writes small JS
 * programs that mix arithmetic (overflow, NaN, -0, coercions), loops, closures, try/catch and
 * finally, property access on shapes that change mid-loop, calls with the wrong argument count,
 * and values whose type changes (the deopt-shaped cases). Each tier runs every program in a child
 * process of this test binary, because the JIT switch is read once per process. The results and
 * the exceptions must be identical across the tiers, and the compiled child must compile, so the
 * comparison cannot pass by the JIT never running. The `eval` used here is the V9 test harness
 * (`common::eval`), which runs each program inside the sandboxed V9 engine, not the host's eval.
 * The case count is `MARS_V9_JIT_FUZZ_CASES`
 * (default `DEFAULT_CASES`), and `MARS_V9_JIT_FUZZ_SEED` picks another stream for local runs.
 */
use crate::common::eval;
use std::process::Command;

/** Set to the results file's path in the child, so it runs the programs instead of spawning more. */
const CHILD_ENV: &str = "MARS_V9_JIT_FUZZ_CHILD";
/** Environment variable that sets how many programs to generate and run. */
const CASES_ENV: &str = "MARS_V9_JIT_FUZZ_CASES";
/** Environment variable that picks the generator's seed. */
const SEED_ENV: &str = "MARS_V9_JIT_FUZZ_SEED";
/** Cases run by default: thousands, enough to reach the shape and type changes in every class. */
const DEFAULT_CASES: usize = 3000;
/** Seed used when the environment does not name one, so the default run is reproducible. */
const DEFAULT_SEED: u64 = 0x9E37_79B9_7F4A_7C15;
/** The compilation report the JIT writes to stderr when tracing is on. */
const COMPILED_REPORT: &str = "V9 JIT compiled";
/** Name of this test as the libtest filter knows it, for the child process. */
const TEST_NAME: &str = "jit_fuzz::jit_matches_interpreter_on_generated_programs";
/** Loop lengths the generator picks from, lower bound and span. */
const LOOP_MIN: usize = 50;
const LOOP_SPAN: usize = 350;
/** How deep generated expressions nest, in the loop body and in helper functions. */
const BODY_DEPTH: usize = 3;
const HELPER_DEPTH: usize = 2;
/** Leaves usable inside the loop body. */
const LOOP_LEAVES: &[&str] = &[
    "i",
    "acc",
    "o.x",
    "o.y",
    "o.z",
    "o['x']",
    "arr[i % 3]",
    "arr.length",
    "f(i)",
    "g(i)",
    "c()",
    "0",
    "-0",
    "1",
    "2147483647",
    "-2147483648",
    "4294967296",
    "9007199254740993",
    "0.5",
    "NaN",
    "Infinity",
    "-Infinity",
    "1e308",
    "'7'",
    "''",
    "'ab'",
    "null",
    "undefined",
    "true",
    "false",
];
/** Leaves usable inside the helper function `f(a, b)`. */
const HELPER_LEAVES: &[&str] = &[
    "a",
    "b",
    "0",
    "-0",
    "1",
    "NaN",
    "'3'",
    "undefined",
    "null",
    "2147483647",
    "-1",
    "0.1",
];
/** Binary operators the generator combines expressions with. */
const BINARY: &[&str] = &[
    "+", "-", "*", "/", "%", "<<", ">>", ">>>", "&", "|", "^", "<", ">", "<=", ">=", "===", "!==",
    "==", "!=", "&&", "||", "??",
];
/** Unary operators, applied in parentheses so a leading sign never fuses with its operand. */
const UNARY: &[&str] = &["-", "!", "~", "+", "typeof "];
/** Property keys the generator indexes objects with, including keys that do not exist. */
const KEYS: &[&str] = &["'x'", "'y'", "'z'", "'w'", "0", "NaN"];

/** A small deterministic generator (xorshift64*), so every run writes the same programs. */
struct Rng(u64);

impl Rng {
    /** A generator for one case; the seed is mixed so neighbouring cases do not share a stream. */
    fn for_case(seed: u64, case: usize) -> Self {
        let mut state = seed ^ (case as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        state ^= state >> 30;
        state = state.wrapping_mul(0x94D0_49BB_1331_11EB);
        Self(if state == 0 { 1 } else { state })
    }
    /** The next 64 random bits. */
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    /** A number in `0..bound`. */
    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }
    /** One of `items`. */
    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[self.below(items.len())]
    }
}

/** An expression over `leaves`, nested at most `depth` levels, always fully parenthesised. */
fn expr(rng: &mut Rng, depth: usize, leaves: &[&str]) -> String {
    if depth == 0 || rng.below(3) == 0 {
        return rng.pick(leaves).to_string();
    }
    let left = expr(rng, depth - 1, leaves);
    match rng.below(7) {
        0 => format!("({}({left}))", rng.pick(UNARY)),
        1 => format!(
            "(({left}) ? ({}) : ({}))",
            expr(rng, depth - 1, leaves),
            expr(rng, depth - 1, leaves)
        ),
        2 => format!("f({left}, {})", expr(rng, depth - 1, leaves)),
        3 => format!("o[({})]", rng.pick(KEYS)),
        4 => format!("f({left})"),
        _ => format!(
            "({left} {} {})",
            rng.pick(BINARY),
            expr(rng, depth - 1, leaves)
        ),
    }
}

/** A whole program. The loop changes the object's shape and the accumulator's type on a schedule,
and every iteration's expression runs inside its own try, so a throw only ends that step. */
fn program(rng: &mut Rng) -> String {
    let loops = LOOP_MIN + rng.below(LOOP_SPAN);
    let switch_at = rng.below(loops);
    let helper = expr(rng, HELPER_DEPTH, HELPER_LEAVES);
    let body = expr(rng, BODY_DEPTH, LOOP_LEAVES);
    let guard = expr(rng, 1, LOOP_LEAVES);
    format!(
        "let r = 'none'; let acc = 0; let cnt = 0; let o = {{x: 1, y: 2}}; const arr = [1, 'two', 3.5];\n\
         function f(a, b) {{ return {helper}; }}\n\
         function g(x) {{ try {{ if (x % 4 === 0) throw new RangeError('r' + x); return x; }} finally {{ cnt++; }} }}\n\
         function mk() {{ let k = 0; return () => {{ k += 1; return k; }}; }}\n\
         const c = mk();\n\
         try {{\n\
           for (let i = 0; i < {loops}; i++) {{\n\
             if (i % 5 === 0) o.z = i;\n\
             if (i % 7 === 0) delete o.x;\n\
             if (i % 11 === 0) o = {{y: i, x: 2}};\n\
             if (i === {switch_at}) acc = 'str' + i;\n\
             try {{ acc = {body}; }} catch (e) {{ acc = 'E:' + e.name + ':' + e.message; }}\n\
             if ({guard}) cnt += 2;\n\
           }}\n\
           r = String(acc) + '|' + (1 / (typeof acc === 'number' ? acc : 1)) + '|' + cnt;\n\
         }} catch (e) {{ r = 'threw ' + e.name + ':' + e.message; }}\n"
    )
}

/** How many cases this run generates, from the environment or the default. */
fn cases() -> usize {
    std::env::var(CASES_ENV)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_CASES)
}

/** The seed this run generates from, from the environment or the default. */
fn seed() -> u64 {
    std::env::var(SEED_ENV)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_SEED)
}

/** The parent runs both tiers and compares them; the child prints one result line per case. */
#[test]
fn jit_matches_interpreter_on_generated_programs() {
    if let Some(results_path) = std::env::var_os(CHILD_ENV) {
        let lines: Vec<String> = (0..cases())
            .map(|case| {
                let source = program(&mut Rng::for_case(seed(), case));
                format!("{case}={}", eval(&source, "r"))
            })
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
    assert_eq!(compiled.len(), cases(), "every case reported a result");
    let divergent: Vec<String> = interpreted
        .iter()
        .zip(&compiled)
        .filter(|(left, right)| left != right)
        .take(10)
        .map(|(left, right)| format!("interpreter {left}\ncompiled    {right}"))
        .collect();
    assert!(
        divergent.is_empty(),
        "the compiled tier diverged from the interpreter:\n{}",
        divergent.join("\n")
    );
    assert!(
        report.contains(COMPILED_REPORT),
        "the JIT must compile functions in this run"
    );
}

/** Runs this test in a child with `environment`, returning its results and its stderr. The
results travel through a file, because the child's stdout shares a line with the harness. The
kill switch is cleared so an operator's `MARS_JIT=0` cannot silently turn the compiled tier off. */
fn run_child(tier: &str, environment: &[(&str, &str)]) -> (Vec<String>, String) {
    let executable = std::env::current_exe().expect("test executable");
    let results_path =
        std::env::temp_dir().join(format!("v9-jit-fuzz-{}-{tier}.txt", std::process::id()));
    let output = Command::new(executable)
        .args(["--exact", TEST_NAME, "--nocapture", "--test-threads=1"])
        .env(CHILD_ENV, &results_path)
        .env_remove("MARS_JIT")
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
