/** IndexedDB cursors and key ranges: directions, continue variants, advance, update and delete. */
use crate::common;
use crate::indexed_db;
use indexed_db::run;

/** A store `s` (keys 1..5, values {k, g}) with an index `g` on g = k % 2, then `body(db)`. */
fn seeded(body: &str) -> String {
    format!(
        r#"let result = '';
        const open = indexedDB.open('db', 1);
        open.onupgradeneeded = () => {{
            const s = open.result.createObjectStore('s');
            s.createIndex('g', 'g');
            for (let k = 1; k <= 5; k++) s.put({{ k, g: k % 2 }}, k);
        }};
        open.onsuccess = () => {{ const db = open.result; {body} }};"#
    )
}

/** Walk a cursor request to the end, appending `describe(cursor)` for each record. */
const WALK: &str = r#"
    function walk(request, describe, done) {
        request.onsuccess = () => {
            const cursor = request.result;
            if (!cursor) { result += '|'; if (done) done(); return; }
            result += describe(cursor) + ',';
            cursor.continue();
        };
    }
"#;

/** Store cursors walk forwards and backwards within a range. */
#[test]
fn store_cursors_follow_direction_and_range() {
    let result = run(&seeded(&format!(
        r#"{WALK}
        const store = db.transaction('s').objectStore('s');
        walk(store.openCursor(IDBKeyRange.bound(2, 4)), (c) => c.key + ':' + c.value.k, () =>
            walk(store.openCursor(null, 'prev'), (c) => c.primaryKey, () =>
                walk(store.openKeyCursor(), (c) => c.key + ':' + ('value' in c) + ':' + c.direction)));
        "#
    )));
    assert_eq!(
        result,
        "2:2,3:3,4:4,|5,4,3,2,1,|1:false:next,2:false:next,3:false:next,4:false:next,5:false:next,|"
    );
}

/** Index cursors order by index key then primary key; unique directions keep the lowest primary key. */
#[test]
fn index_cursor_directions() {
    let result = run(&seeded(&format!(
        r#"{WALK}
        const index = db.transaction('s').objectStore('s').index('g');
        const show = (c) => c.key + '/' + c.primaryKey;
        walk(index.openCursor(), show, () => walk(index.openCursor(null, 'prev'), show, () =>
            walk(index.openCursor(null, 'nextunique'), show, () => walk(index.openKeyCursor(null, 'prevunique'), show))));
        "#
    )));
    assert_eq!(
        result,
        "0/2,0/4,1/1,1/3,1/5,|1/5,1/3,1/1,0/4,0/2,|0/2,1/1,|1/1,0/2,|"
    );
}

/** continue(key), continuePrimaryKey and advance jump; going backwards is a DataError. */
#[test]
fn continue_variants_and_advance() {
    let result = run(&seeded(
        r#"
        const store = db.transaction('s').objectStore('s');
        const request = store.openCursor();
        let step = 0;
        request.onsuccess = () => {
            const cursor = request.result;
            if (!cursor) { result += 'end|'; return; }
            result += cursor.key + ',';
            if (step++ === 0) {
                try { cursor.continue(1); } catch (e) { result += e.name + ','; }
                cursor.continue(3);
            } else if (step === 2) {
                cursor.advance(2);
            } else {
                cursor.continue();
            }
        };
        const index = db.transaction('s').objectStore('s').index('g');
        const indexed = index.openCursor();
        let first = true;
        indexed.onsuccess = () => {
            const cursor = indexed.result;
            if (!cursor) return;
            indexSteps += cursor.key + '/' + cursor.primaryKey + ',';
            if (first) { first = false; cursor.continuePrimaryKey(1, 3); }
        };
        let indexSteps = '';
        setTimeout(() => setTimeout(() => { result += indexSteps; }, 0), 0);
    "#,
    ));
    assert_eq!(result, "1,DataError,3,5,end|0/2,1/3,");
}

/** cursor.update and cursor.delete change the record under the cursor. */
#[test]
fn cursor_update_and_delete() {
    let result = run(&seeded(
        r#"
        const tx = db.transaction('s', 'readwrite');
        const store = tx.objectStore('s');
        const request = store.openCursor();
        request.onsuccess = () => {
            const cursor = request.result;
            if (!cursor) return;
            if (cursor.key % 2) cursor.update({ k: cursor.key * 10, g: 9 }); else cursor.delete();
            cursor.continue();
        };
        tx.oncomplete = () => {
            const read = db.transaction('s').objectStore('s');
            read.getAll().onsuccess = (e) => { result += e.target.result.map((v) => v.k).join(',') + '|'; };
            read.index('g').count(9).onsuccess = (e) => { result += e.target.result; };
        };
    "#,
    ));
    assert_eq!(result, "10,30,50|3");
}

/** Cursor misuse: update in a read-only transaction, a key cursor's update, advance(0). */
#[test]
fn cursor_misuse_errors() {
    let result = run(&seeded(
        r#"
        const store = db.transaction('s').objectStore('s');
        const request = store.openKeyCursor();
        request.onsuccess = () => {
            const cursor = request.result;
            if (result) return;
            for (const attempt of [() => cursor.update({}), () => cursor.advance(0), () => { cursor.continue(); cursor.continue(); }]) {
                try { attempt(); } catch (e) { result += e.name + ','; }
            }
        };
    "#,
    ));
    assert_eq!(result, "ReadOnlyError,TypeError,InvalidStateError,");
}

/** IDBKeyRange: bounds, open flags, includes, and invalid ranges. */
#[test]
fn key_ranges() {
    let result = common::eval(
        r#"
        const r = IDBKeyRange.bound('a', 'c', true, false);
        const parts = [r.lower, r.upper, r.lowerOpen, r.upperOpen, r.includes('a'), r.includes('b'), r.includes('c'),
            IDBKeyRange.only(1).includes(1), IDBKeyRange.lowerBound(5).upper, IDBKeyRange.upperBound([1]).upper[0]];
        for (const bad of [() => IDBKeyRange.bound(2, 1), () => IDBKeyRange.bound(1, 1, true), () => IDBKeyRange.only({}), () => new IDBKeyRange()]) {
            try { bad(); } catch (e) { parts.push(e.name); }
        }
        const result = parts.join(',') + ',' + Object.prototype.toString.call(r);
    "#,
        "result",
    );
    assert_eq!(
        result,
        "a,c,true,false,false,true,true,true,,1,DataError,DataError,DataError,TypeError,[object IDBKeyRange]"
    );
}

/** Binary and date keys come back as ArrayBuffer and Date; cursor keys are stable objects. */
#[test]
fn binary_and_date_keys_round_trip() {
    let result = run(r#"
        let result = '';
        const open = indexedDB.open('db', 1);
        open.onupgradeneeded = () => open.result.createObjectStore('s');
        open.onsuccess = () => {
            const store = open.result.transaction('s', 'readwrite').objectStore('s');
            store.put('bin', new Uint8Array([1, 2]));
            store.put('date', new Date(7));
            store.put('arr', [1, 'x']);
            const request = store.openCursor();
            request.onsuccess = () => {
                const c = request.result;
                if (!c) return;
                const k = c.key;
                result += (k instanceof ArrayBuffer ? 'buf' + new Uint8Array(k).join('.') : k instanceof Date ? 'date' + k.getTime() : JSON.stringify(k)) + (c.key === k) + ',';
                c.continue();
            };
        };
    "#);
    assert_eq!(result, "date7true,buf1.2true,[1,\"x\"]true,");
}
