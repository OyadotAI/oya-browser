/** IndexedDB transactions: active-flag lifetime, auto-commit, abort and rollback, error propagation and scheduling. */
mod common;
mod indexed_db;
use indexed_db::run;

/** The setup every test shares: database `db` version 1 with an out-of-line store `s`, then `body(db)`. */
fn with_store(body: &str) -> String {
    format!(
        r#"let result = '';
        const open = indexedDB.open('db', 1);
        open.onupgradeneeded = () => open.result.createObjectStore('s');
        open.onsuccess = () => {{ const db = open.result; {body} }};"#
    )
}

/** A transaction is inactive once its creating task ends; a later task gets TransactionInactiveError. */
#[test]
fn a_transaction_is_inactive_after_its_task() {
    let result = run(&with_store(
        r#"
        const tx = db.transaction('s', 'readwrite');
        const store = tx.objectStore('s');
        Promise.resolve().then(() => { store.put(1, 'micro'); result += 'micro-ok|'; });
        setTimeout(() => {
            try { store.put(1, 'late'); } catch (e) { result += e.name; }
        }, 0);
    "#,
    ));
    assert_eq!(result, "micro-ok|TransactionInactiveError");
}

/** Promise continuations inside a success handler run while the transaction is still active. */
#[test]
fn promise_callbacks_in_handlers_keep_the_transaction_usable() {
    let result = run(&with_store(
        r#"
        const store = db.transaction('s', 'readwrite').objectStore('s');
        const wrap = (request) => new Promise((resolve) => { request.onsuccess = () => resolve(request.result); });
        (async () => {
            await wrap(store.put('a', 1));
            await wrap(store.put('b', 2));
            result = String(await wrap(store.count()));
        })();
    "#,
    ));
    assert_eq!(result, "2");
}

/** abort() rolls every change back, fails pending requests with AbortError and fires abort with a null error. */
#[test]
fn abort_rolls_back_and_reports() {
    let result = run(&with_store(
        r#"
        const tx = db.transaction('s', 'readwrite');
        const store = tx.objectStore('s');
        store.put('x', 1).onsuccess = () => {
            const pending = store.put('y', 2);
            pending.onerror = (e) => { result += 'request:' + pending.error.name + ':' + e.bubbles + '|'; };
            tx.abort();
        };
        tx.onabort = () => {
            result += 'abort:' + tx.error + '|';
            db.transaction('s').objectStore('s').count().onsuccess = (e) => { result += 'count:' + e.target.result; };
        };
    "#,
    ));
    assert_eq!(result, "request:AbortError:true|abort:null|count:0");
}

/** An unhandled request error aborts its transaction with that error; preventDefault keeps it alive. */
#[test]
fn request_errors_abort_unless_prevented() {
    let result = run(&with_store(
        r#"
        const first = db.transaction('s', 'readwrite');
        first.objectStore('s').add(1, 'k');
        first.objectStore('s').add(2, 'k');
        first.onabort = () => {
            result += 'aborted:' + first.error.name + '|';
            const second = db.transaction('s', 'readwrite');
            second.objectStore('s').add(1, 'k');
            second.objectStore('s').add(2, 'k').onerror = (e) => e.preventDefault();
            second.oncomplete = () => {
                db.transaction('s').objectStore('s').get('k').onsuccess = (e) => { result += 'kept:' + e.target.result; };
            };
        };
    "#,
    ));
    assert_eq!(result, "aborted:ConstraintError|kept:1");
}

/** Error events capture and bubble through request, transaction and database in order. */
#[test]
fn error_events_propagate_request_transaction_database() {
    let result = run(&with_store(
        r#"
        const tx = db.transaction('s', 'readwrite');
        const store = tx.objectStore('s');
        store.add(1, 'k');
        const failing = store.add(2, 'k');
        db.addEventListener('error', () => { result += 'db-capture,'; }, true);
        tx.addEventListener('error', () => { result += 'tx-capture,'; }, true);
        failing.onerror = (e) => { result += 'request:' + (e.target === failing) + ','; };
        tx.onerror = (e) => { result += 'tx:' + (e.target === failing) + ','; };
        db.onerror = (e) => { result += 'db:' + e.eventPhase + ',' + e.isTrusted; e.preventDefault(); };
    "#,
    ));
    assert_eq!(
        result,
        "db-capture,tx-capture,request:true,tx:true,db:3,true"
    );
}

/** Writes in a read-only transaction throw ReadOnlyError synchronously. */
#[test]
fn readonly_transactions_refuse_writes() {
    let result = run(&with_store(
        r#"
        const store = db.transaction('s').objectStore('s');
        for (const write of [() => store.put(1, 1), () => store.add(1, 1), () => store.delete(1), () => store.clear()]) {
            try { write(); } catch (e) { result += e.name + ','; }
        }
    "#,
    ));
    assert_eq!(
        result,
        "ReadOnlyError,ReadOnlyError,ReadOnlyError,ReadOnlyError,"
    );
}

/** commit() ends the request phase at once; complete still follows the queued requests. */
#[test]
fn explicit_commit_stops_new_requests() {
    let result = run(&with_store(
        r#"
        const tx = db.transaction('s', 'readwrite');
        tx.objectStore('s').put(1, 1).onsuccess = () => { result += 'put,'; };
        tx.commit();
        try { tx.objectStore('s').put(2, 2); } catch (e) { result += e.name + ','; }
        try { tx.commit(); } catch (e) { result += e.name + ','; }
        tx.oncomplete = () => { result += 'complete'; };
    "#,
    ));
    assert_eq!(
        result,
        "TransactionInactiveError,InvalidStateError,put,complete"
    );
}

/** A later read/write transaction over the same store waits for the earlier one to finish. */
#[test]
fn overlapping_writers_run_in_creation_order() {
    let result = run(&with_store(
        r#"
        const first = db.transaction('s', 'readwrite');
        const second = db.transaction('s', 'readwrite');
        second.objectStore('s').get('k').onsuccess = (e) => { result += 'second-saw:' + e.target.result + ','; };
        first.objectStore('s').put('one', 'k').onsuccess = () => { result += 'first-put,'; };
        first.oncomplete = () => { result += 'first-complete,'; };
        second.oncomplete = () => { result += 'second-complete'; };
    "#,
    ));
    assert_eq!(
        result,
        "first-put,first-complete,second-saw:one,second-complete"
    );
}

/** A success listener that throws aborts the transaction with AbortError. */
#[test]
fn a_throwing_success_listener_aborts() {
    let result = run(&with_store(
        r#"
        const tx = db.transaction('s', 'readwrite');
        tx.objectStore('s').put(1, 1).onsuccess = () => { throw new Error('boom'); };
        tx.onabort = () => { result = tx.error.name; };
    "#,
    ));
    assert_eq!(result, "AbortError");
}

/** An aborted upgrade restores the old version and schema, and the open request fails with AbortError. */
#[test]
fn an_aborted_upgrade_is_rolled_back() {
    let result = run(r#"
        let result = '';
        const first = indexedDB.open('db', 1);
        first.onupgradeneeded = () => first.result.createObjectStore('keep');
        first.onsuccess = () => {
            first.result.close();
            const second = indexedDB.open('db', 2);
            second.onupgradeneeded = () => {
                const db = second.result;
                db.createObjectStore('dropped');
                db.deleteObjectStore('keep');
                second.transaction.abort();
                result += 'version-after-abort:' + db.version + ',' + [...db.objectStoreNames].join('+') + '|';
            };
            second.onerror = (e) => {
                result += second.error.name + '|';
                e.preventDefault();
                const third = indexedDB.open('db');
                third.onsuccess = () => { result += third.result.version + ':' + [...third.result.objectStoreNames].join('+'); };
            };
        };
    "#);
    assert_eq!(result, "version-after-abort:1,keep|AbortError|1:keep");
}

/** Other connections get versionchange; one that stays open makes the request blocked until it closes. */
#[test]
fn version_change_notifies_and_blocks_until_closed() {
    let result = run(r#"
        let result = '';
        const first = indexedDB.open('db', 1);
        first.onsuccess = () => {
            const old = first.result;
            old.onversionchange = (e) => {
                result += 'versionchange:' + e.oldVersion + '>' + e.newVersion + ',';
                setTimeout(() => { result += 'closing,'; old.close(); }, 0);
            };
            const second = indexedDB.open('db', 2);
            second.onblocked = (e) => { result += 'blocked:' + e.newVersion + ','; };
            second.onupgradeneeded = () => { result += 'upgrade,'; };
            second.onsuccess = () => { result += 'success'; };
        };
    "#);
    assert_eq!(
        result,
        "versionchange:1>2,blocked:2,closing,upgrade,success"
    );
}

/** A transaction with no requests still commits and fires complete. */
#[test]
fn empty_transactions_auto_commit() {
    let result = run(&with_store(
        r#"
        const tx = db.transaction(['s'], 'readonly', { durability: 'relaxed' });
        result += tx.mode + ',' + tx.durability + ',' + tx.objectStoreNames[0] + ',' + (tx.db === db) + ',';
        tx.oncomplete = () => { result += 'complete'; };
    "#,
    ));
    assert_eq!(result, "readonly,relaxed,s,true,complete");
}

/** transaction() argument errors: unknown store, empty scope, bad mode, closing connection. */
#[test]
fn transaction_argument_errors() {
    let result = run(&with_store(
        r#"
        for (const attempt of [() => db.transaction('missing'), () => db.transaction([]), () => db.transaction('s', 'versionchange')]) {
            try { attempt(); } catch (e) { result += e.name + ','; }
        }
        db.close();
        try { db.transaction('s'); } catch (e) { result += e.name; }
    "#,
    ));
    assert_eq!(
        result,
        "NotFoundError,InvalidAccessError,TypeError,InvalidStateError"
    );
}
