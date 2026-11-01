/** Dynamic code a page keeps across steps: a step that re-adopts a retained eval or Function
sees its own locals and reports its own source, exactly as a step that compiled it would. */
use crate::common;
use mars_v9::{Execution, VmState};

/** The page's button, the target every click in these tests runs. */
const BUTTON: usize = 3;

/** One click, run against the state the previous step left in memory (as the app carries it,
not through a JSON round trip, which would drop the compiled code). */
fn click(sources: &[String], state: VmState) -> Execution {
    let host = common::host();
    let environment = common::environment();
    mars_v9::click_page(common::input(sources, &host, &environment), state, BUTTON).unwrap()
}

/** Loads the sources, then clicks `clicks` times in a row; each click's console lines, joined
with newlines. */
fn clicks(sources: &[String], clicks: usize) -> Vec<String> {
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", loaded.failed);
    let mut state = loaded.state;
    let mut lines = Vec::new();
    for _ in 0..clicks {
        let clicked = click(sources, state);
        assert!(clicked.failed.is_empty(), "{:?}", clicked.failed);
        lines.push(common::console(&clicked).join("\n"));
        state = clicked.state;
    }
    lines
}

/** A direct eval compiled by the load step reads the caller's locals of each later step: the
locals change between steps, and the retained eval code must not hold on to the first ones. */
#[test]
fn retained_direct_eval_sees_each_steps_locals() {
    let sources = vec![
        "let n=0;function f(a){let b=a+1;return eval('a*10+b')}console.log(f(0));\
        document.getElementById('go').addEventListener('click',()=>{n=n+1;console.log(f(n))});"
            .to_string(),
    ];
    assert_eq!(clicks(&sources, 3), ["12", "23", "34"]);
}

/** Two Functions compiled by the load step keep their own source text and their own code when
later steps re-adopt both: each reports and runs its own body. */
#[test]
fn retained_functions_keep_their_own_source_after_reuse() {
    let sources = vec![
        "const one=new Function('a','return a+1 /*one*/');\
        const two=new Function('a','return a+2 /*two*/');\
        document.getElementById('go').addEventListener('click',()=>console.log(\
        [one.toString(),two.toString(),one(1),two(1)].join('|')));"
            .to_string(),
    ];
    let expected = "function anonymous(a\n) {\nreturn a+1 /*one*/\n}|function anonymous(a\n) {\nreturn a+2 /*two*/\n}|2|3";
    assert_eq!(clicks(&sources, 2), [expected, expected]);
}
