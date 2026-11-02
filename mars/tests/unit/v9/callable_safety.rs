/** Public callable boundaries enforce shared work, graph, metadata and aggregate allocation limits. */
use mars::v9::{event, execute, load};

/** Observable script results use native document proposals rather than private evaluator hooks. */
fn result(source: &str, expression: &str) -> String {
    execute(&format!(
        "{source}document.getElementById('out').textContent={expression};"
    ))
    .unwrap()
    .last()
    .unwrap()
    .text
    .clone()
}

/** Recursion through call adapters stops at the call depth with a catchable RangeError. */
#[test]
fn recursive_call_adapters_hit_the_call_depth() {
    let error = execute("function recur(){return recur.call(null);}recur();").unwrap_err();
    assert_eq!(error.code, "script_execution_error");
    assert!(error.message.contains("RangeError"), "{}", error.message);
    assert_eq!(
        result(
            "let caught=false;function recur(){return recur.call(null);}try{recur();}catch(error){caught=error instanceof RangeError;}",
            "caught"
        ),
        "true"
    );
}

/**
 * A self-referential apply list must stop at the call depth like any recursion, as a catchable
 * RangeError, rather than overflowing the V9 thread's native stack and aborting the process.
 */
#[test]
fn self_referential_apply_lists_hit_the_call_depth() {
    let error = execute("const apply=Function.prototype.apply;const args=[apply];args[1]=args;apply.apply(apply,args);").unwrap_err();
    assert_eq!(error.code, "script_execution_error");
    assert!(error.message.contains("RangeError"), "{}", error.message);
}

/** Long bind chains are ordinary functions: each link is one live function, and the chain still calls through. */
#[test]
fn long_bind_chains_invoke_their_target() {
    let source = "let f=function(){return 7;};for(let i=0;i<200;i=i+1){f=f.bind(null);}";
    assert_eq!(result(source, "f()+' '+f.call(null)"), "7 7");
}

/// Properties one event adds (half on an object, half on a function): enough that the page
/// allowance is reached in a few dozen events, since each event clones the whole heap.
const PROPERTIES_PER_EVENT: usize = 8192;

/** Objects and function expandos consume one property allowance and failed events preserve earlier records. */
#[test]
fn function_and_object_properties_share_a_page_quota_across_events() {
    let source = &format!(
        "const box={{}};function f(){{}}let n=0;function run(){{for(let i=0;i<{};i=i+1){{box['a'+n]=n;f['b'+n]=n;n=n+1;}}document.getElementById('out').textContent=n;}}function read(){{document.getElementById('out').textContent=n;}}",
        PROPERTIES_PER_EVENT / 2
    );
    let mut state = load(source, &[]).unwrap().state;
    let mut committed = String::new();
    for _ in 0..mars::constants::VM_PROPERTIES / PROPERTIES_PER_EVENT + 2 {
        match event(source, "run()", state.clone(), &[]) {
            Ok(executed) => {
                committed = executed.changes[0].text().unwrap().to_owned();
                state = executed.state;
            }
            Err(error) => {
                assert_eq!(error.code, "limit_exceeded");
                // The keys are themselves distinct strings, so the page's string units reach
                // their bound at nearly the same count as the property quota; either may trip.
                assert!(
                    error.message.contains("properties") || error.message.contains("string"),
                    "{}",
                    error.message
                );
                assert!(!committed.is_empty());
                assert_eq!(
                    event(source, "read()", state, &[]).unwrap().changes[0]
                        .text()
                        .unwrap(),
                    committed
                );
                return;
            }
        }
    }
    panic!("Function and object properties exceeded the shared page allowance.");
}
