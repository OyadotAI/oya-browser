/** A failed generator resume entry closes the generator without leaving a false running state or entering its body. */
use crate::common;

/** Repeated near-limit retries see a completed generator after its real resume entry overflows. */
#[test]
fn initial_generator_resume_overflow_closes_without_entering_body() {
    let source = "let entered=0,ranges=0,other=0;function* run(){entered++;yield 1}const it=run();function dive(){try{dive()}catch(error){if(error instanceof RangeError)ranges++;else other++;it.next()}}dive();const marker={};let same=false;try{it.throw(marker)}catch(error){same=error===marker}const returned=it.return(7);const r=[ranges>0,other,entered,it.next().done,same,returned.value,returned.done].join();";
    assert_eq!(common::eval(source, "r"), "true,0,0,true,true,7,true");
}

/** A yielded generator whose resume cannot enter is closed before its protected body or finally can run. */
#[test]
fn suspended_generator_resume_overflow_does_not_run_body_or_finally() {
    let source = "let resumed=0,cleaned=0,ranges=0,other=0;function* run(){try{yield 1;resumed++}finally{cleaned++}}const it=run();const first=it.next();function dive(){try{dive()}catch(error){if(error instanceof RangeError)ranges++;else other++;it.next()}}dive();const r=[first.value,first.done,ranges>0,other,resumed,cleaned,it.next().done].join();";
    assert_eq!(common::eval(source, "r"), "1,false,true,0,0,0,true");
}

/** Actual recursive entry while a generator body executes still throws TypeError and leaves the outer resume intact. */
#[test]
fn genuinely_running_generators_still_reject_reentrant_resumes() {
    let source = "const out=[];function* run(){for(const call of [()=>it.next(),()=>it.return(7),()=>it.throw(8)]){try{call();out.push(false)}catch(error){out.push(error instanceof TypeError,error.message==='Generator is already running')}}yield 1;return 2}const it=run();const first=it.next(),second=it.next();out.push(first.value,first.done,second.value,second.done,it.next().done);const r=out.join();";
    assert_eq!(
        common::eval(source, "r"),
        "true,true,true,true,true,true,1,false,2,true,true"
    );
}

/** Closing after entry overflow leaves a valid completed generator across public JSON transport. */
#[test]
fn completed_generator_after_entry_failure_survives_transport() {
    let sources=vec!["function* run(){yield 'unexpected'}const it=run();function dive(){try{dive()}catch(error){it.next()}}dive();document.getElementById('go').addEventListener('click',()=>{const next=it.next(),returned=it.return(9);console.log(next.done,next.value,returned.done,returned.value)});".into()];
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
    assert_eq!(common::console(&clicked), ["true undefined true 9"]);
}
