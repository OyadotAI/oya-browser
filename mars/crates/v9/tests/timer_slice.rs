/** A timer run gives way once its time slice is spent, so one uninterruptible request cannot
run every due callback of a busy page (YouTube) while a person's click waits. */
mod common;

/** Three due callbacks that each outlast the slice run one per request, in order, none lost. */
#[test]
fn timer_run_stops_after_its_slice_and_later_runs_finish_the_rest() {
    let sources = vec![
        "for(let i=0;i<3;i++)setTimeout(()=>{let x=0;for(let j=0;j<1000000;j++)x+=j;\
         console.log('t'+i)},0);"
            .to_string(),
    ];
    let environment = common::environment();
    let host = common::host();
    let mut state = mars_v9::load_page(common::input(&sources, &host, &environment))
        .unwrap()
        .state;
    let mut runs = Vec::new();
    for _ in 0..3 {
        let ran = mars_v9::run_timers(
            common::input(&sources, &host, &environment),
            common::round_trip(state),
            5.0,
            10,
        )
        .unwrap();
        runs.push(common::console(&ran));
        state = ran.state;
    }
    assert!(runs[0].len() < 3, "{runs:?}");
    assert_eq!(runs.concat(), ["t0", "t1", "t2"]);
}
