/** Date local conversion uses validated transitions and POSIX recurrence while keeping test clocks hermetic. */
use crate::common;

/** Synthetic northern transitions choose the earlier fold and normalize a skipped wall hour forwards. */
#[test]
fn local_gaps_and_folds_use_transition_offsets() {
    let mut environment = common::environment();
    environment.timezone_initial_offset_seconds = Some(-18000);
    environment.timezone_transitions = vec![(1710054000000.0, -14400), (1730613600000.0, -18000)];
    let execution = common::load_with(
        "const gap=new Date(2024,2,10,2,30);const fold=new Date(2024,10,3,1,30);const r=[gap.toISOString(),gap.getHours(),gap.getTimezoneOffset(),fold.toISOString(),fold.getTimezoneOffset(),new Date(NaN).getTimezoneOffset()].join('|');",
        &environment,
    );
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(
        common::global(&execution, "r"),
        "2024-03-10T07:30:00.000Z|3|240|2024-11-03T05:30:00.000Z|240|NaN"
    );
}

/** A half-hour displacement uses its actual width rather than assuming every daylight transition is an hour. */
#[test]
fn half_hour_gaps_preserve_their_real_displacement() {
    let mut environment = common::environment();
    environment.timezone_initial_offset_seconds = Some(37800);
    environment.timezone_transitions = vec![(1728142200000.0, 39600)];
    let execution = common::load_with(
        "const d=new Date(2024,9,6,2,15);const r=[d.toISOString(),d.getHours(),d.getMinutes()].join('|');",
        &environment,
    );
    assert!(execution.failed.is_empty());
    assert_eq!(
        common::global(&execution, "r"),
        "2024-10-05T15:45:00.000Z|2|45"
    );
}

/** Recurring rules handle northern and southern DST without relying on operating-system timezone state. */
#[test]
fn posix_recurrence_handles_both_hemispheres() {
    let mut environment = common::environment();
    environment.timezone_posix_rule = Some("EST5EDT,M3.2.0,M11.1.0".into());
    let execution = common::load_with(
        "const a=new Date(2050,0,1);const b=new Date(2050,6,1);const r=[a.getUTCHours(),a.getTimezoneOffset(),b.getUTCHours(),b.getTimezoneOffset()].join();",
        &environment,
    );
    assert!(execution.failed.is_empty());
    assert_eq!(common::global(&execution, "r"), "5,300,4,240");
    environment.timezone_posix_rule = Some("AEST-10AEDT-11,M10.1.0,M4.1.0/3".into());
    let execution = common::load_with(
        "const a=new Date(Date.UTC(2050,0,1));const b=new Date(Date.UTC(2050,6,1));const r=[a.getHours(),a.getTimezoneOffset(),b.getHours(),b.getTimezoneOffset()].join();",
        &environment,
    );
    assert!(execution.failed.is_empty());
    assert_eq!(common::global(&execution, "r"), "11,-660,10,-600");
}

/** Date setters resolve the new local wall time against its own transition rather than the old instant's offset. */
#[test]
fn setters_recompute_offset_for_the_resulting_wall_time() {
    let mut environment = common::environment();
    environment.timezone_posix_rule = Some("EST5EDT,M3.2.0,M11.1.0".into());
    let execution = common::load_with(
        "const d=new Date(2024,0,10,12);d.setMonth(6);const r=[d.getHours(),d.getUTCHours(),d.getTimezoneOffset()].join();",
        &environment,
    );
    assert!(execution.failed.is_empty());
    assert_eq!(common::global(&execution, "r"), "12,16,240");
}

/** Invalid transition tables and malformed recurrence text are rejected before any script can execute. */
#[test]
fn malformed_timezone_metadata_is_rejected_before_execution() {
    let sources = vec!["throw 7;".to_string()];
    let host = common::host();
    for transitions in [
        vec![(1.0, 0), (0.0, 1)],
        vec![(0.0, 86401)],
        vec![(f64::NAN, 0)],
    ] {
        let mut environment = common::environment();
        environment.timezone_transitions = transitions;
        assert!(mars_v9::load_page(common::input(&sources, &host, &environment)).is_err());
    }
    let mut environment = common::environment();
    environment.timezone_posix_rule = Some("EST5EDT,M0.2.0,M11.1.0".into());
    assert!(mars_v9::load_page(common::input(&sources, &host, &environment)).is_err());
}
