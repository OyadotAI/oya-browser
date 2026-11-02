
  /**
   * IDBDatabase: one connection to a database. A connection closes once close() was called and
   * its last transaction finished; closing wakes any open or delete request waiting on it.
   */

  const IDBDatabase = makeInterface('IDBDatabase', NativeEventTarget);
  /** Transaction modes a script may request. */
  const MODES = ['readonly', 'readwrite'];
  /** Durability hints a script may request. */
  const DURABILITIES = ['default', 'strict', 'relaxed'];

  /** A new open connection to a database model. */
  function newConnection(db) {
    const connection = {
      brand: IDBDatabase, db, name: db.name, version: db.version, closePending: false, closed: false,
      upgrade: null, transactions: new NativeSet(), closedNames: null,
    };
    connection.object = eventTargetInstance(IDBDatabase, connection);
    db.connections.add(connection);
    return connection;
  }

  /** Close a connection whose close is pending once its transactions are done, and wake waiters. */
  function connectionSettled(connection) {
    if (!connection.closePending || connection.closed || connection.transactions.size > 0) {
      return;
    }
    connection.closed = true;
    connection.closedNames = sortedNames(connection.db.stores.keys());
    connection.db.connections.delete(connection);
    wakeWaiters(connection.db);
    releaseDatabase(connection.db.name);
  }

  /** Run every waiter whose condition now holds, each in its own task. */
  function wakeWaiters(db) {
    const waiting = db.waiters || [];
    db.waiters = waiting.filter((waiter) => !waiter.ready());
    for (const waiter of waiting) {
      if (!db.waiters.includes(waiter)) {
        queueTask(waiter.then);
      }
    }
  }

  /** Run `then` once `ready()` holds, now or after a connection closes. */
  function waitUntil(db, ready, then) {
    if (ready()) {
      then();
      return;
    }
    db.waiters = db.waiters || [];
    db.waiters.push({ ready, then });
  }

  members(IDBDatabase.prototype, {
    get name() { return stateOf(this, IDBDatabase).name; },
    get version() { return stateOf(this, IDBDatabase).version; },
    get objectStoreNames() {
      const connection = stateOf(this, IDBDatabase);
      return stringList(connection.closedNames || sortedNames(connection.db.stores.keys()));
    },
    createObjectStore(name, options = undefined) {
      requireArgument(arguments, 1, 'createObjectStore', 'IDBDatabase');
      return createStoreOn(stateOf(this, IDBDatabase), String(name), options);
    },
    deleteObjectStore(name) {
      requireArgument(arguments, 1, 'deleteObjectStore', 'IDBDatabase');
      deleteStoreOn(stateOf(this, IDBDatabase), String(name));
    },
    transaction(storeNames, mode = undefined, options = undefined) {
      requireArgument(arguments, 1, 'transaction', 'IDBDatabase');
      return startTransaction(stateOf(this, IDBDatabase), storeNames, mode, options).object;
    },
    close() {
      const connection = stateOf(this, IDBDatabase);
      connection.closePending = true;
      connectionSettled(connection);
    },
  });
  eventHandlers(IDBDatabase.prototype, ['abort', 'close', 'error', 'versionchange']);

  /** transaction()'s WebIDL argument conversions: store names, mode and durability. */
  function transactionArguments(storeNames, modeInput, options) {
    const names = typeof storeNames !== 'string' && isObject(storeNames) && typeof storeNames[Symbol.iterator] === 'function'
      ? [...storeNames].map((name) => String(name)) : [String(storeNames)];
    const mode = enumArgument(modeInput, 'readonly', MODES, 'IDBTransactionMode');
    const dictionary = options === undefined || options === null ? {} : options;
    const durability = enumArgument(dictionary.durability, 'default', DURABILITIES, 'IDBTransactionDurability');
    return { names, mode, durability };
  }

  /** transaction() is refused while an upgrade runs or the connection is closing. */
  function requireOpenConnection(connection) {
    if (connection.upgrade && connection.upgrade.state !== 'finished') {
      throw stateError('A version change transaction is running.');
    }
    if (connection.closePending) {
      throw stateError('The database connection is closing.');
    }
  }

  /** IDBDatabase.transaction's steps: argument conversion first, then the state checks in order. */
  function startTransaction(connection, storeNames, modeInput, options) {
    const { names, mode, durability } = transactionArguments(storeNames, modeInput, options);
    requireOpenConnection(connection);
    const scope = sortedNames(new NativeSet(names));
    if (scope.some((name) => !connection.db.stores.has(name))) {
      throw domError('NotFoundError', 'One of the specified object stores was not found.');
    }
    if (scope.length === 0) {
      throw domError('InvalidAccessError', 'The storeNames parameter was empty.');
    }
    const transaction = newTransaction(connection, mode, new NativeSet(scope.map((name) => connection.db.stores.get(name))), durability);
    transaction.scopeNames = scope;
    return transaction;
  }

  /** WebIDL enum conversion: undefined takes the default, anything outside the values is a TypeError. */
  function enumArgument(value, fallback, values, type) {
    const text = value === undefined ? fallback : String(value);
    if (!values.includes(text)) {
      throw new NativeTypeError(`The provided value '${text}' is not a valid enum value of type ${type}.`);
    }
    return text;
  }
