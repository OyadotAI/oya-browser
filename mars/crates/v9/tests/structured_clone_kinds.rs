/** Structured clone across contexts (worker, port and BroadcastChannel packets) and structuredClone keep Date, RegExp, Map, Set, wrappers, errors and Blobs. */
mod common;
use mars_v9::{BroadcastCommand, PageEvent};

/** A message with every richer kind; the receiver reports what arrived. */
const SOURCE: &str = r#"
    const a = new BroadcastChannel('room'), b = new BroadcastChannel('room');
    let result = 'pending';
    b.onmessage = (e) => {
        const d = e.data;
        result = [d.date instanceof Date && d.date.getTime(), d.regexp.source + d.regexp.flags, d.map.get(1).x, d.map.get(1) === d.again,
            d.set.has('a'), typeof d.number === 'object' && d.number + 1, d.error instanceof TypeError && d.error.message,
            d.file instanceof File && d.file.name + ':' + d.file.type + ':' + d.file.size].join('|');
    };
    const shared = { x: 2 };
    a.postMessage({ date: new Date(5), regexp: /x+/gi, map: new Map([[1, shared]]), again: shared, set: new Set(['a']),
        number: new Number(3), error: new TypeError('bad'), file: new File(['hi'], 'f.txt', { type: 'text/plain' }) });
"#;

/** The packet survives the wire and revives every kind, keeping shared references shared. */
#[test]
fn messages_carry_dates_maps_sets_wrappers_errors_and_files() {
    let execution = common::load(SOURCE);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    let id = execution.effects.broadcast_channels[1].id;
    let data = execution
        .effects
        .broadcast_commands
        .iter()
        .find_map(|command| match command {
            BroadcastCommand::Post { data, .. } => Some(data.clone()),
            _ => None,
        })
        .expect("a posted packet");
    let data = serde_json::from_value(serde_json::to_value(data).unwrap()).unwrap();
    let host = common::host();
    let environment = common::environment();
    let sources = vec![SOURCE.to_string()];
    let delivered = mars_v9::page_event(
        common::input(&sources, &host, &environment),
        common::round_trip(execution.state),
        PageEvent::BroadcastMessage {
            id,
            data,
            origin: "https://example.com".into(),
        },
    )
    .unwrap();
    assert_eq!(
        common::global(&delivered, "result"),
        "5|x+gi|2|true|true|4|bad|f.txt:text/plain:2"
    );
}

/** structuredClone copies Blobs and Files as Blobs and Files, not plain objects. */
#[test]
fn structured_clone_copies_blobs() {
    let result = common::eval(
        "const f = structuredClone(new File(['abc'], 'n', { type: 'text/plain', lastModified: 7 })); const b = structuredClone(new Blob(['xy'])); const result = [f instanceof File, f.name, f.size, f.type, f.lastModified, b instanceof Blob, b instanceof File, b.size].join(',');",
        "result",
    );
    assert_eq!(result, "true,n,3,text/plain,7,true,false,2");
}
