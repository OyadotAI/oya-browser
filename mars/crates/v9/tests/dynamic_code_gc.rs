/** Dynamic code pressure collects unreachable source units while retaining closure identity and validated source locations. */
mod common;

/** Repeated distinct temporary functions release code instead of accumulating execution history against the source allowance. */
#[test]
fn discarded_dynamic_functions_release_their_source_units() {
    assert_eq!(
        common::eval(
            "const pad='x'.repeat(3000);const keep=Function('return 42');let last;for(let i=0;i<900;i++){last=Function('/*'+pad+'*/return '+i);if(last()!==i)throw 7}const r=[keep(),last(),keep.toString().includes('return 42')].join();",
            "r"
        ),
        "42,899,true"
    );
}

/** Direct eval's temporary compilation units release source after its caller resumes. */
#[test]
fn discarded_eval_units_release_their_sources() {
    assert_eq!(
        common::eval(
            "const pad='x'.repeat(3000);let last;for(let i=0;i<900;i++){last=eval('/*'+pad+'*/'+i)}const r=last;",
            "r"
        ),
        "899"
    );
}

/** A surviving early closure and a suspended generator still find their original code after interior unit reclamation. */
#[test]
fn retained_dynamic_code_survives_collection_and_transport() {
    let sources=vec!["const keep=Function('return 42');const make=Function('return function*(){yield 1;yield 2}');const g=make()();g.next();const pad='x'.repeat(3000);for(let i=0;i<900;i++){Function('/*'+pad+'*/return '+i)()}document.getElementById('go').onclick=()=>console.log(keep(),g.next().value);".to_string()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let clicked = mars_v9::click_page(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        3,
    )
    .unwrap();
    assert!(clicked.failed.is_empty(), "{:?}", common::failure(&clicked));
    assert_eq!(common::console(&clicked), ["42 2"]);
}

/** Untrusted explicit source offsets cannot make two separately compiled programs own the same function sites. */
#[test]
fn overlapping_saved_source_ranges_are_rejected() {
    let sources=vec!["const a=Function('return 1');const b=Function('return 2');document.getElementById('go').onclick=()=>console.log(a(),b());".to_string()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty());
    let mut json = serde_json::to_value(loaded.state).unwrap();
    let base = json["dynamic"][0][2].clone();
    json["dynamic"][1][2] = base;
    let state = serde_json::from_value(json).unwrap();
    assert!(mars_v9::click_page(common::input(&sources, &host, &environment), state, 3).is_err());
}
