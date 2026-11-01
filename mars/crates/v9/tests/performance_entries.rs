/** Performance marks and measures store real timeline entries across serialized page requests. */
use crate::common;

/** Marks, measures, retrieval and clearing share the same underlying timeline. */
#[test]
fn marks_and_measures_are_recorded_and_cleared() {
    let source = "const a=performance.mark('a'),b=performance.mark('b'),m=performance.measure('span','a','b');let missing=false;try{performance.measure('bad','missing')}catch(e){missing=true}const before=[a.entryType,b.name,m.startTime===a.startTime,m.duration===b.startTime-a.startTime,performance.getEntriesByType('mark').length,performance.getEntriesByName('span').length,missing];performance.clearMarks('a');performance.clearMeasures();const r=[...before,performance.getEntries().length,performance.getEntries()[0].name].join();";
    assert_eq!(common::eval(source, "r"), "mark,b,true,true,2,1,true,1,b");
}

/** A borrowed method cannot mutate an unrelated object's private timing storage. */
#[test]
fn performance_methods_require_the_real_receiver() {
    let source = "let rejected=0;for(const name of ['mark','measure','getEntries','clearMarks'])try{performance[name].call({},'x')}catch(e){rejected+=e instanceof TypeError}const r=rejected;";
    assert_eq!(common::eval(source, "r"), "4");
}

/** Required names and types are checked without inventing an undefined-named entry. */
#[test]
fn performance_methods_require_arguments() {
    let source = "let rejected=0;for(const name of ['mark','measure','getEntriesByName','getEntriesByType'])try{performance[name]()}catch(e){rejected+=e instanceof TypeError}const r=[rejected,performance.getEntries().length].join();";
    assert_eq!(common::eval(source, "r"), "4,0");
}

/** Name conversion can create a mark without the outer call discarding that entry. */
#[test]
fn nested_mark_during_name_conversion_is_preserved() {
    let source = "performance.mark({toString(){performance.mark('inner');return 'outer'}});const r=performance.getEntriesByType('mark').map(x=>x.name).join();";
    assert_eq!(common::eval(source, "r"), "inner,outer");
}

/** The page's timing entries survive a JSON round trip and remain available to later tasks. */
#[test]
fn marks_survive_page_state_transport() {
    let sources = vec!["performance.mark('before');document.getElementById('go').addEventListener('click',()=>{const m=performance.measure('later','before');console.log(m.name,performance.getEntries().length)})".into()];
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
    assert_eq!(common::console(&clicked), ["later 2"]);
}
