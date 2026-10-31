/** IndexedDB object stores and indexes: keys, key paths, key generators, constraints, ranges and schema changes. */
mod common;
mod indexed_db;
use indexed_db::run;

/** Open `db` version 1, run `upgrade(db)` in upgradeneeded and `body(db)` on success. */
fn database(upgrade: &str, body: &str) -> String {
    format!(
        r#"let result = '';
        const open = indexedDB.open('db', 1);
        open.onupgradeneeded = () => {{ const db = open.result; {upgrade} }};
        open.onsuccess = () => {{ const db = open.result; {body} }};"#
    )
}

/** Key generators number records, inject in-line keys, and jump past explicit numeric keys. */
#[test]
fn key_generator_injects_and_advances() {
    let result = run(&database(
        "db.createObjectStore('s', { keyPath: 'a.id', autoIncrement: true });",
        r#"
        const store = db.transaction('s', 'readwrite').objectStore('s');
        store.add({ a: {} }).onsuccess = (e) => { result += e.target.result + ','; };
        store.add({ a: { id: 10.5 } }).onsuccess = (e) => { result += e.target.result + ','; };
        store.add({}).onsuccess = (e) => { result += e.target.result + ','; };
        store.get(11).onsuccess = (e) => { result += JSON.stringify(e.target.result) + ',' + store.autoIncrement + ',' + store.keyPath; };
        "#,
    ));
    assert_eq!(result, "1,10.5,11,{\"a\":{\"id\":11}},true,a.id");
}

/** add() refuses an existing key with ConstraintError; put() replaces. */
#[test]
fn add_refuses_existing_keys_and_put_replaces() {
    let result = run(&database(
        "db.createObjectStore('s');",
        r#"
        const tx = db.transaction('s', 'readwrite');
        const store = tx.objectStore('s');
        store.put('a', 1);
        store.put('b', 1);
        const dup = store.add('c', 1);
        dup.onerror = (e) => { result += dup.error.name + ','; e.preventDefault(); };
        store.get(1).onsuccess = (e) => { result += e.target.result; };
        "#,
    ));
    assert_eq!(result, "ConstraintError,b");
}

/** Synchronous argument errors: invalid keys, keys for in-line stores, missing keys, uncloneable values. */
#[test]
fn put_validates_keys_and_values_synchronously() {
    let result = run(&database(
        "db.createObjectStore('out'); db.createObjectStore('in', { keyPath: 'id' });",
        r#"
        const tx = db.transaction(['out', 'in'], 'readwrite');
        const out = tx.objectStore('out');
        const inline = tx.objectStore('in');
        for (const attempt of [
            () => out.put(1, NaN), () => out.put(1, {}), () => out.put(1), () => inline.put({ id: 1 }, 1),
            () => inline.put({ id: true }), () => inline.put({}), () => out.put(() => 1, 1), () => out.get(undefined), () => out.get(),
        ]) {
            try { attempt(); result += 'ok,'; } catch (e) { result += e.name + ','; }
        }
        "#,
    ));
    assert_eq!(
        result,
        "DataError,DataError,DataError,DataError,DataError,DataError,DataCloneError,DataError,TypeError,"
    );
}

/** Array key paths build compound keys. */
#[test]
fn array_key_paths_make_compound_keys() {
    let result = run(&database(
        "db.createObjectStore('s', { keyPath: ['last', 'first'] });",
        r#"
        const store = db.transaction('s', 'readwrite').objectStore('s');
        store.put({ last: 'b', first: 'x' });
        store.put({ last: 'a', first: 'z' });
        store.put({ last: 'a', first: 'y' });
        store.getAllKeys().onsuccess = (e) => { result = JSON.stringify(e.target.result) + ',' + JSON.stringify(store.keyPath) + ',' + (store.keyPath === store.keyPath); };
        "#,
    ));
    assert_eq!(
        result,
        "[[\"a\",\"y\"],[\"a\",\"z\"],[\"b\",\"x\"]],[\"last\",\"first\"],true"
    );
}

/** getAll, getAllKeys, count and delete honour ranges, counts and directions. */
#[test]
fn ranged_reads_and_deletes() {
    let result = run(&database(
        "const s = db.createObjectStore('s'); for (let i = 1; i <= 6; i++) s.put('v' + i, i);",
        r#"
        const store = db.transaction('s', 'readwrite').objectStore('s');
        store.getAll(IDBKeyRange.bound(2, 5, true, false), 2).onsuccess = (e) => { result += e.target.result + '|'; };
        store.getAllKeys({ query: IDBKeyRange.lowerBound(3), direction: 'prev', count: 2 }).onsuccess = (e) => { result += e.target.result + '|'; };
        store.count(IDBKeyRange.upperBound(3, true)).onsuccess = (e) => { result += e.target.result + '|'; };
        store.delete(IDBKeyRange.bound(2, 4));
        store.getAllKeys().onsuccess = (e) => { result += e.target.result + '|'; };
        store.getAllRecords({ query: 5 }).onsuccess = (e) => { const r = e.target.result[0]; result += r.key + ':' + r.primaryKey + ':' + r.value + '|'; };
        store.clear();
        store.count().onsuccess = (e) => { result += e.target.result; };
        "#,
    ));
    assert_eq!(result, "v3,v4|6,5|2|1,5,6|5:5:v5|0");
}

/** Indexes answer by index key; unique indexes refuse duplicates; multiEntry indexes each array element. */
#[test]
fn indexes_unique_and_multi_entry() {
    let result = run(&database(
        r#"const s = db.createObjectStore('s', { keyPath: 'id' });
           s.createIndex('email', 'email', { unique: true });
           s.createIndex('tags', 'tags', { multiEntry: true });"#,
        r#"
        const tx = db.transaction('s', 'readwrite');
        const store = tx.objectStore('s');
        store.put({ id: 1, email: 'a@x', tags: ['red', 'blue', 'red'] });
        store.put({ id: 2, email: 'b@x', tags: ['blue'] });
        const dup = store.put({ id: 3, email: 'a@x', tags: [] });
        dup.onerror = (e) => { result += dup.error.name + ','; e.preventDefault(); };
        const email = store.index('email');
        email.get('b@x').onsuccess = (e) => { result += e.target.result.id + ','; };
        store.index('tags').getAllKeys('blue').onsuccess = (e) => { result += e.target.result + ','; };
        store.index('tags').count('red').onsuccess = (e) => { result += e.target.result + ','; };
        email.getKey('a@x').onsuccess = (e) => { result += e.target.result + ',' + email.unique + email.multiEntry + email.keyPath + ',' + [...store.indexNames].join('+'); };
        "#,
    ));
    assert_eq!(
        result,
        "ConstraintError,2,1,2,1,1,truefalseemail,email+tags"
    );
}

/** Creating a unique index over records that break it aborts the upgrade with ConstraintError. */
#[test]
fn a_unique_index_over_duplicates_aborts_the_upgrade() {
    let result = run(r#"
        let result = '';
        const first = indexedDB.open('db', 1);
        first.onupgradeneeded = () => { const s = first.result.createObjectStore('s'); s.put({ v: 1 }, 1); s.put({ v: 1 }, 2); };
        first.onsuccess = () => {
            first.result.close();
            const second = indexedDB.open('db', 2);
            second.onupgradeneeded = () => {
                second.transaction.objectStore('s').createIndex('v', 'v', { unique: true });
                second.transaction.onabort = () => { result += second.transaction.error.name + ','; };
            };
            second.onerror = (e) => { result += second.error.name; e.preventDefault(); };
        };
    "#);
    assert_eq!(result, "ConstraintError,AbortError");
}

/** Stores and indexes can be renamed during an upgrade, and the new names persist. */
#[test]
fn renames_during_upgrade_persist() {
    let mut origin = mars_v9::IndexedDbOrigin::default();
    let first = r#"
        let result = '';
        const open = indexedDB.open('db', 1);
        open.onupgradeneeded = () => { const s = open.result.createObjectStore('old'); s.createIndex('i', 'x'); s.put({ x: 1 }, 'k'); };
        open.onsuccess = () => { open.result.close(); result = 'v1'; };
    "#;
    assert_eq!(indexed_db::run_with(first, &mut origin), "v1");
    let second = r#"
        let result = '';
        const open = indexedDB.open('db', 2);
        open.onupgradeneeded = () => {
            const store = open.transaction.objectStore('old');
            store.name = 'new';
            store.index('i').name = 'j';
            try { store.name = 'new'; open.result.createObjectStore('other').name = 'new'; } catch (e) { result += e.name + ','; }
        };
        open.onsuccess = () => {
            const store = open.result.transaction('new').objectStore('new');
            store.index('j').get(1).onsuccess = (e) => { result += [...open.result.objectStoreNames].join('+') + ',' + e.target.result.x; };
        };
    "#;
    assert_eq!(
        indexed_db::run_with(second, &mut origin),
        "ConstraintError,new+other,1"
    );
}

/** Schema methods outside an upgrade throw InvalidStateError; unknown names throw NotFoundError. */
#[test]
fn schema_methods_need_an_upgrade() {
    let result = run(&database(
        "db.createObjectStore('s');",
        r#"
        const store = db.transaction('s', 'readwrite').objectStore('s');
        for (const attempt of [() => db.createObjectStore('t'), () => db.deleteObjectStore('s'), () => store.createIndex('i', 'x'), () => store.index('nope')]) {
            try { attempt(); } catch (e) { result += e.name + ','; }
        }
        "#,
    ));
    assert_eq!(
        result,
        "InvalidStateError,InvalidStateError,InvalidStateError,NotFoundError,"
    );
}

/** createObjectStore validates names, key paths and autoIncrement combinations. */
#[test]
fn create_object_store_validates_options() {
    let result = run(r#"
        let result = '';
        const open = indexedDB.open('db', 1);
        open.onupgradeneeded = () => {
            const db = open.result;
            db.createObjectStore('s');
            for (const attempt of [
                () => db.createObjectStore('s'), () => db.createObjectStore('t', { keyPath: 'a b' }),
                () => db.createObjectStore('u', { keyPath: '', autoIncrement: true }), () => db.createObjectStore('v', { keyPath: ['a'], autoIncrement: true }),
            ]) {
                try { attempt(); } catch (e) { result += e.name + ','; }
            }
        };
    "#);
    assert_eq!(
        result,
        "ConstraintError,SyntaxError,InvalidAccessError,InvalidAccessError,"
    );
}
