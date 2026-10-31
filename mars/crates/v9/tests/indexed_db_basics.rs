/** IndexedDB: opening, upgrading, reading and writing through the page's engine and a test coordinator. */
use crate::common;
use crate::indexed_db;
use indexed_db::run;

/** Open creates the database in an upgrade, then succeeds with a connection at the requested version. */
#[test]
fn open_runs_upgradeneeded_then_success_in_order() {
    let result = run(r#"
        let result = '';
        const request = indexedDB.open('db', 2);
        result += request.readyState + '|';
        request.onupgradeneeded = (e) => {
            result += 'upgrade:' + e.oldVersion + '>' + e.newVersion + ':' + (e instanceof IDBVersionChangeEvent) + ':' + request.transaction.mode + '|';
            request.result.createObjectStore('s');
        };
        request.onsuccess = () => {
            const db = request.result;
            result += 'success:' + db.name + ':' + db.version + ':' + db.objectStoreNames.length + ':' + db.objectStoreNames[0] + ':' + (request.transaction === null);
        };
    "#);
    assert_eq!(
        result,
        "pending|upgrade:0>2:true:versionchange|success:db:2:1:s:true"
    );
}

/** Put then get round-trips a structured value; get hands out a fresh clone. */
#[test]
fn put_and_get_round_trip_structured_values() {
    let result = run(r#"
        let result = '';
        const open = indexedDB.open('db');
        open.onupgradeneeded = () => open.result.createObjectStore('s');
        open.onsuccess = () => {
            const db = open.result;
            const value = { n: 1, d: new Date(5), m: new Map([[1, 'a']]), s: new Set([2]), b: new Uint8Array([7, 8]), r: /x/gi };
            value.self = value;
            const tx = db.transaction('s', 'readwrite');
            const store = tx.objectStore('s');
            store.put(value, 'k').onsuccess = (e) => { result += 'put:' + e.target.result + '|'; };
            store.get('k').onsuccess = (e) => {
                const v = e.target.result;
                result += [v.n, v.d.getTime(), v.m.get(1), v.s.has(2), v.b[1], v.r.flags, v.self === v, v !== value].join(',') + '|';
            };
            tx.oncomplete = () => { result += 'complete'; };
        };
    "#);
    assert_eq!(result, "put:k|1,5,a,true,8,gi,true,true|complete");
}

/** A version below the stored one fails with VersionError; 0 and negative versions throw TypeError. */
#[test]
fn version_rules_reject_downgrades_and_zero() {
    let result = run(r#"
        let result = '';
        for (const bad of [0, -1, NaN]) {
            try { indexedDB.open('db', bad); } catch (e) { result += e.name + ','; }
        }
        const first = indexedDB.open('db', 3);
        first.onsuccess = () => {
            first.result.close();
            const lower = indexedDB.open('db', 2);
            lower.onerror = (e) => { result += lower.error.name + ':' + e.type; e.preventDefault(); };
            const same = indexedDB.open('db');
            same.onsuccess = () => { result += '|' + same.result.version; };
        };
    "#);
    assert_eq!(result, "TypeError,TypeError,TypeError,VersionError:error|3");
}

/** Data committed by one page load is there for the next load of the same origin. */
#[test]
fn committed_data_persists_across_page_loads() {
    let mut origin = mars_v9::IndexedDbOrigin::default();
    let write = r#"
        let result = '';
        const open = indexedDB.open('notes', 1);
        open.onupgradeneeded = () => open.result.createObjectStore('n', { keyPath: 'id', autoIncrement: true }).createIndex('by_tag', 'tag');
        open.onsuccess = () => {
            const tx = open.result.transaction('n', 'readwrite');
            tx.objectStore('n').add({ tag: 'x', text: 'hello', when: new Date(9), blob: new Blob(['bytes'], { type: 'text/plain' }) });
            tx.objectStore('n').add({ tag: 'y', text: 'bye' });
            tx.oncomplete = () => { result = 'written'; };
        };
    "#;
    assert_eq!(indexed_db::run_with(write, &mut origin), "written");
    let read = r#"
        let result = '';
        const open = indexedDB.open('notes');
        open.onsuccess = () => {
            const db = open.result;
            const store = db.transaction('n').objectStore('n');
            store.index('by_tag').get('x').onsuccess = (e) => {
                const v = e.target.result;
                result = [db.version, v.id, v.text, v.when.getTime(), v.blob instanceof Blob, v.blob.type, v.blob.size].join(',');
                v.blob.text().then((text) => { result += ',' + text; });
            };
            store.count().onsuccess = (e) => { result += ',' + e.target.result; };
        };
    "#;
    assert_eq!(
        indexed_db::run_with(read, &mut origin),
        "1,1,hello,9,true,text/plain,5,bytes,2"
    );
}

/** deleteDatabase reports the old version, and a later open starts from version 0. */
#[test]
fn delete_database_removes_it() {
    let result = run(r#"
        let result = '';
        const open = indexedDB.open('db', 4);
        open.onsuccess = () => {
            open.result.close();
            const del = indexedDB.deleteDatabase('db');
            del.onsuccess = (e) => {
                result += 'deleted:' + e.oldVersion + ':' + e.newVersion + ':' + del.result + '|';
                const again = indexedDB.open('db');
                again.onupgradeneeded = (u) => { result += 'fresh:' + u.oldVersion; };
            };
        };
    "#);
    assert_eq!(result, "deleted:4:null:undefined|fresh:0");
}

/** databases() lists every database the origin holds with its version. */
#[test]
fn databases_lists_names_and_versions() {
    let result = run(r#"
        let result = '';
        const a = indexedDB.open('a', 2);
        a.onsuccess = () => {
            const b = indexedDB.open('b', 5);
            b.onsuccess = () => indexedDB.databases().then((list) => {
                result = list.map((d) => d.name + '@' + d.version).sort().join(',');
            });
        };
    "#);
    assert_eq!(result, "a@2,b@5");
}

/** indexedDB.cmp orders number < date < string < binary < array and rejects invalid keys. */
#[test]
fn cmp_follows_key_order() {
    let result = common::eval(
        r#"
        const ordered = [-Infinity, -1, -0, 1e300, Infinity, new Date(0), new Date(1), '', 'a', 'a\u0000', 'b', new Uint8Array([]).buffer, new Uint8Array([0]), new Uint8Array([1]), [], [0], [0, 0], [1], ['a']];
        const pairs = [];
        for (let i = 0; i + 1 < ordered.length; i++) pairs.push(indexedDB.cmp(ordered[i], ordered[i + 1]));
        let error = '';
        try { indexedDB.cmp(NaN, 1); } catch (e) { error = e.name; }
        const result = pairs.join('') + '|' + indexedDB.cmp(-0, 0) + indexedDB.cmp([1, 'x'], [1, 'x']) + indexedDB.cmp('b', 'a') + '|' + error;
    "#,
        "result",
    );
    assert_eq!(result, "-1".repeat(18) + "|001|DataError");
}
