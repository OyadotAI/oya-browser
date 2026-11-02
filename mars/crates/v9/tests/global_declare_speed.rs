/** Declaring many vars stays linear: half the variable quota of short names, as global `var`s
in one statement and as a sloppy direct eval's vars inside a function, finishes well inside the
request's wall-clock budget through the legacy `execute` path. */
use mars_v9::{constants::VM_BINDINGS, execute};
use std::time::{Duration, Instant};

/** Generous: each workload takes well under two seconds in release; the regression this pins
hit the request's four-second time limit. */
const DECLARE_BUDGET: Duration = Duration::from_secs(3);

/** Short distinct names, `prefix` then the index in base 36, joined with commas. */
fn names(prefix: &str, range: std::ops::Range<usize>) -> String {
    range
        .map(|index| {
            let mut digits = Vec::new();
            let mut value = index;
            loop {
                digits.push(char::from_digit((value % 36) as u32, 36).unwrap());
                value /= 36;
                if value == 0 {
                    break;
                }
            }
            format!("{prefix}{}", digits.into_iter().rev().collect::<String>())
        })
        .collect::<Vec<_>>()
        .join(",")
}

/** Run `source` through `execute`, asserting success within the budget in release builds. */
fn within_budget(source: &str) {
    let started = Instant::now();
    let result = execute(source);
    let spent = started.elapsed();
    eprintln!("ran in {spent:?}");
    assert!(result.is_ok(), "{:?}", result.err());
    if cfg!(not(debug_assertions)) {
        assert!(spent < DECLARE_BUDGET, "took {spent:?}");
    }
}

/** Half of VM_BINDINGS globals in one `var` statement run within the budget. */
#[test]
fn many_global_var_declarations_stay_within_budget() {
    within_budget(&format!("var {};", names("$", 0..VM_BINDINGS / 2)));
}

/** Half of VM_BINDINGS vars declared by direct evals inside one function run within the budget. */
#[test]
fn many_eval_var_declarations_stay_within_budget() {
    let evals = VM_BINDINGS / 2;
    let parts = 4;
    let calls = (0..parts)
        .map(|part| {
            let range = part * evals / parts..(part + 1) * evals / parts;
            format!("eval('var {}');", names("_", range))
        })
        .collect::<String>();
    within_budget(&format!("function f(){{{calls}}}f();"));
}
