
  /**
   * IDBTransaction and the transaction lifecycle. A transaction is active while the task that
   * created it runs and while one of its request events is dispatched; it is inactive otherwise,
   * committing once commit() was called or its last request finished while inactive, and
   * finished after complete or abort. Its state record is the IDBTransaction's internal state.
   */

  const IDBTransaction = makeInterface('IDBTransaction', NativeEventTarget);
  /** Transactions created in the current task; the end-of-task cleanup makes them inactive. */
  const freshTransactions = new NativeSet();

  /** Create a transaction over `scope` (store records) on a connection. */
  function newTransaction(connection, mode, scope, durability) {
    const state = {
      brand: IDBTransaction, connection, db: connection.db, mode, durability, scope, state: 'active',
      requests: [], started: false, undo: [], dirty: new NativeMap(), cleared: new NativeSet(), meta: new NativeSet(),
      error: null, handles: new NativeMap(), finished: [], aborted: false, scopeNames: null,
    };
    state.object = eventTargetInstance(IDBTransaction, state);
    host.parent(state.object, connection.object);
    connection.transactions.add(state);
    connection.db.transactions.push(state);
    busy.add(connection.db);
    freshTransactions.add(state);
    host.pending();
    schedule();
    return state;
  }

  /** The end-of-task cleanup ("cleanup Indexed Database transactions"): fresh transactions become inactive. */
  function cleanupTransactions() {
    for (const transaction of freshTransactions) {
      if (transaction.state === 'active') {
        transaction.state = 'inactive';
      }
    }
    freshTransactions.clear();
  }

  /** Throw TransactionInactiveError unless the transaction accepts requests now. */
  function requireActive(transaction) {
    if (transaction.state !== 'active') {
      throw inactiveError();
    }
  }

  /** Throw ReadOnlyError for a write in a read-only transaction. */
  function requireWritable(transaction) {
    if (transaction.mode === 'readonly') {
      throw domError('ReadOnlyError', 'The transaction is read-only.');
    }
  }

  /** Queue an operation as a request on `transaction`; the scheduler runs it in order. */
  function queueRequest(transaction, source, operation, request) {
    const target = request || newRequest(source, transaction, false);
    transaction.requests.push({ request: target, operation });
    schedule();
    return target;
  }

  /** The names in a transaction's scope, sorted; an upgrade transaction sees the live store set. */
  function scopeNames(transaction) {
    if (transaction.mode === 'versionchange') {
      return sortedNames(transaction.db.stores.keys());
    }
    return transaction.scopeNames;
  }

  members(IDBTransaction.prototype, {
    get objectStoreNames() { return stringList(scopeNames(stateOf(this, IDBTransaction))); },
    get mode() { return stateOf(this, IDBTransaction).mode; },
    get durability() { return stateOf(this, IDBTransaction).durability; },
    get db() { return stateOf(this, IDBTransaction).connection.object; },
    get error() { return stateOf(this, IDBTransaction).error; },
    objectStore(name) {
      const transaction = stateOf(this, IDBTransaction);
      requireArgument(arguments, 1, 'objectStore', 'IDBTransaction');
      if (transaction.state === 'finished') {
        throw stateError('The transaction has finished.');
      }
      const store = transaction.mode === 'versionchange' ? transaction.db.stores.get(String(name)) : scopeStore(transaction, String(name));
      if (!store) {
        throw domError('NotFoundError', 'The specified object store was not found.');
      }
      return storeHandle(transaction, store);
    },
    abort() {
      const transaction = stateOf(this, IDBTransaction);
      if (transaction.state === 'committing' || transaction.state === 'finished') {
        throw stateError('The transaction has already committed or aborted.');
      }
      abortTransaction(transaction, null);
    },
    commit() {
      const transaction = stateOf(this, IDBTransaction);
      if (transaction.state !== 'active') {
        throw stateError('The transaction is not active.');
      }
      transaction.state = 'committing';
      schedule();
    },
  });
  eventHandlers(IDBTransaction.prototype, ['abort', 'complete', 'error']);

  /** The store named `name` inside a non-upgrade transaction's scope. */
  function scopeStore(transaction, name) {
    for (const store of transaction.scope) {
      if (store.name === name && !store.deleted) {
        return store;
      }
    }
    return undefined;
  }

  /** The one IDBObjectStore object a transaction hands out for a store. */
  function storeHandle(transaction, store) {
    let handle = call(mapGet, transaction.handles, store);
    if (!handle) {
      handle = newStoreHandle(transaction, store);
      transaction.handles.set(store, handle);
    }
    return handle;
  }

  /** Throw the TypeError WebIDL gives a call with too few arguments. */
  function requireArgument(args, count, operation, interfaceName) {
    if (args.length < count) {
      throw new NativeTypeError(`Failed to execute '${operation}' on '${interfaceName}': ${count} argument${count > 1 ? 's' : ''} required, but only ${args.length} present.`);
    }
  }

  /**
   * "Abort a transaction": undo every change, settle the requests that never ran with AbortError
   * (each fires error in a later task), then fire abort. An upgrade transaction also restores
   * the connection and lets the open request report AbortError.
   */
  function abortTransaction(transaction, error) {
    if (transaction.state === 'finished') {
      return;
    }
    rollback(transaction);
    transaction.state = 'finished';
    transaction.aborted = true;
    transaction.error = error;
    const pending = transaction.requests;
    transaction.requests = [];
    for (const { request } of pending) {
      fail(request, domError('AbortError', 'The transaction was aborted.'));
      queueTask(() => fireEvent(request.object, 'error', true, true));
    }
    queueTask(() => {
      fireEvent(transaction.object, 'abort', true, false);
      finishTransaction(transaction);
    });
  }

  /** Run the undo journal newest first. */
  function rollback(transaction) {
    const undo = transaction.undo;
    transaction.undo = [];
    for (let index = undo.length - 1; index >= 0; index--) {
      undo[index]();
    }
  }

  /** Forget a finished transaction and wake whatever waited for it (closing connections, open requests). */
  function finishTransaction(transaction) {
    const db = transaction.db;
    const position = db.transactions.indexOf(transaction);
    if (position >= 0) {
      db.transactions.splice(position, 1);
    }
    transaction.connection.transactions.delete(transaction);
    for (const callback of transaction.finished) {
      callback();
    }
    transaction.finished = [];
    connectionSettled(transaction.connection);
    schedule();
  }
