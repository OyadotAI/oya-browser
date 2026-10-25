/** Array elements with their own attributes or accessors (V8's dictionary elements): defined
through Object.defineProperty they keep their descriptor, and length, delete, keys and the
Array methods all respect it. */
mod common;
use common::eval;

/** An accessor element runs its getter and setter through every read and write path. */
#[test]
fn accessor_elements_run_their_getter_and_setter() {
    let source = "const a = [1, 2, 3]; let set = 0;
        Object.defineProperty(a, '1', { get() { return 20; }, set(v) { set = v; }, configurable: true });
        a[1] = 7;
        const out = [a[1], set, a.indexOf(20), a.map(x => x * 2).join(), [...a].join(),
            Math.max.apply(null, a), typeof Object.getOwnPropertyDescriptor(a, '1').get].join();";
    assert_eq!(eval(source, "out"), "20,7,1,2,40,6,1,20,3,20,function");
}

/** A new element defined with no attributes is read-only, hidden and permanent. */
#[test]
fn absent_attributes_default_to_false_on_a_new_element() {
    let source = "const a = []; Object.defineProperty(a, '0', { value: 5 });
        a[0] = 9;
        const d = Object.getOwnPropertyDescriptor(a, '0');
        const out = [a[0], a.length, d.writable, d.enumerable, d.configurable, delete a[0],
            Object.keys(a).length].join();";
    assert_eq!(eval(source, "out"), "5,1,false,false,false,false,0");
}

/** Shrinking length stops above a non-configurable element (ArraySetLength). */
#[test]
fn length_stops_above_a_non_configurable_element() {
    let source = "'use strict'; const a = [0, 1, 2, 3];
        Object.defineProperty(a, '1', { value: 1, configurable: false });
        let threw = false;
        try { a.length = 0; } catch (e) { threw = e instanceof TypeError; }
        const out = [a.length, threw, Object.keys(a).join('')].join();";
    assert_eq!(eval(source, "out"), "2,true,01");
}

/** Frozen arrays reject writes, and own keys stay in index order. */
#[test]
fn frozen_arrays_reject_writes_and_keys_stay_ordered() {
    let source = "const a = Object.freeze([1, 2]); a[0] = 5;
        const b = [0, 1, 2]; Object.defineProperty(b, '1', { value: 'x', enumerable: true, writable: true });
        const out = [a[0], Object.isFrozen(a), Reflect.ownKeys(b).join(), delete b[0], b[0]].join();";
    assert_eq!(eval(source, "out"), "1,true,0,1,2,length,true,");
}

/** A frozen or accessor array survives the state round trip a later task resumes from: its
property-list elements sit over holes, which the state validator accepts as genuine. */
#[test]
fn dictionary_elements_survive_state_round_trip() {
    let sources = vec![
        "const frozen = Object.freeze([1, 2]);
        const b = [0, 1]; Object.defineProperty(b, '1', { get() { return 9; } });
        fetch('/x').then(() => console.log(frozen.join(), Object.isFrozen(frozen), b[1]));"
            .to_string(),
    ];
    let environment = common::environment();
    let host = common::host();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    let id = loaded.effects.requests[0].id;
    let resumed = mars_v9::resume_requests(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        vec![mars_v9::HostResponse {
            id,
            status: 200,
            status_text: "OK".into(),
            url: "https://example.com/x".into(),
            ..mars_v9::HostResponse::default()
        }],
    )
    .expect("state with dictionary elements validates");
    assert_eq!(common::console(&resumed), ["1,2 true 9"]);
}
