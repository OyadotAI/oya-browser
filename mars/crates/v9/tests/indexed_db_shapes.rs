/** IndexedDB interfaces have Chrome's shape: illegal constructors, enumerable WebIDL members, branded receivers, native-looking source text. */
mod common;

/** Interface objects throw on construction (except IDBVersionChangeEvent), and members are enumerable accessors and methods. */
#[test]
fn interfaces_have_webidl_shapes() {
    let result = common::eval(
        r#"
        const parts = [];
        for (const name of ['IDBFactory', 'IDBDatabase', 'IDBRequest', 'IDBCursor', 'IDBObjectStore', 'DOMStringList']) {
            try { new globalThis[name](); parts.push(name + ':constructed'); } catch (e) { parts.push(e.name); }
        }
        const getter = Object.getOwnPropertyDescriptor(IDBRequest.prototype, 'result');
        const method = Object.getOwnPropertyDescriptor(IDBObjectStore.prototype, 'put');
        parts.push(getter.enumerable && typeof getter.get, getter.get.name, method.enumerable && method.writable, IDBObjectStore.prototype.put.length,
            IDBDatabase.prototype.transaction.length, IDBKeyRange.bound.length, Object.getPrototypeOf(IDBOpenDBRequest.prototype) === IDBRequest.prototype,
            Object.getPrototypeOf(IDBRequest.prototype) === EventTarget.prototype, String(IDBFactory.prototype.open), Object.keys(window).includes('IDBFactory'));
        const event = new IDBVersionChangeEvent('versionchange', { oldVersion: 1, newVersion: 2 });
        parts.push(event instanceof Event, event.oldVersion, event.newVersion, event.type, Object.prototype.toString.call(event));
        try { IDBRequest.prototype.readyState; } catch (e) { parts.push(e.name); }
        try { Object.getOwnPropertyDescriptor(IDBRequest.prototype, 'result').get.call({}); } catch (e) { parts.push(e.name); }
        const result = parts.join(',');
    "#,
        "result",
    );
    assert_eq!(
        result,
        "TypeError,TypeError,TypeError,TypeError,TypeError,TypeError,function,get result,true,1,1,2,true,true,function open() { [native code] },false,true,1,2,versionchange,[object IDBVersionChangeEvent],TypeError,TypeError"
    );
}

/** indexedDB is one factory, exposed as an accessor beside the other storage attributes. */
#[test]
fn indexed_db_attribute_is_a_singleton_accessor() {
    let result = common::eval(
        r#"
        let holder = window;
        while (holder && !Object.getOwnPropertyDescriptor(holder, 'indexedDB')) holder = Object.getPrototypeOf(holder);
        const descriptor = Object.getOwnPropertyDescriptor(holder, 'indexedDB');
        const result = [indexedDB === window.indexedDB, indexedDB instanceof IDBFactory, typeof descriptor.get, descriptor.set, String(indexedDB)].join(',');
    "#,
        "result",
    );
    assert_eq!(result, "true,true,function,,[object IDBFactory]");
}

/** An opaque origin has no IndexedDB: open throws SecurityError. */
#[test]
fn opaque_origins_are_refused() {
    let environment = mars_v9::PageEnvironment {
        url: "data:text/html,x".into(),
        ..common::environment()
    };
    let execution = common::load_with(
        "let result = ''; try { indexedDB.open('x'); } catch (e) { result = e.name; }",
        &environment,
    );
    assert_eq!(common::global(&execution, "result"), "SecurityError");
}
