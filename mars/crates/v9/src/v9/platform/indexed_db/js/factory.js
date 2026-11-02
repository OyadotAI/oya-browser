
  /**
   * IDBFactory: open, deleteDatabase, databases and cmp. Open and delete requests for one name
   * wait in a connection queue and run one at a time. A database is loaded from the coordinator
   * on first use and kept while it has connections or queued requests; once released, the next
   * open loads it again, so it sees what other pages committed meanwhile.
   */

  const IDBFactory = makeInterface('IDBFactory', null);
  /** Loaded databases by name. */
  const registry = new NativeMap();
  /** Pending open and delete operations by database name, the running one first. */
  const connectionQueues = new NativeMap();

  /** Add an operation to a name's connection queue; the first one starts in a later task. */
  function enqueue(name, operation) {
    let queue = call(mapGet, connectionQueues, name);
    if (!queue) {
      queue = [];
      connectionQueues.set(name, queue);
    }
    queue.push(operation);
    if (queue.length === 1) {
      queueTask(operation);
    }
  }

  /** The running operation finished: start the next one, or release the database. */
  function dequeue(name) {
    const queue = call(mapGet, connectionQueues, name);
    queue.shift();
    if (queue.length > 0) {
      queueTask(queue[0]);
      return;
    }
    connectionQueues.delete(name);
    releaseDatabase(name);
  }

  /** Forget a loaded database nothing uses, so the next open reads the committed copy. */
  function releaseDatabase(name) {
    const db = call(mapGet, registry, name);
    if (db && db.connections.size === 0 && db.transactions.length === 0 && !connectionQueues.has(name)) {
      registry.delete(name);
    }
  }

  /** Call `then(db, error)` with the loaded database (null when none exists) in a later task. */
  function withDatabase(name, then) {
    const loaded = call(mapGet, registry, name);
    if (loaded) {
      queueTask(() => then(loaded, null));
      return;
    }
    const queued = host.load(wireName(name), (image) => {
      queueTask(() => (typeof image === 'string' ? then(loadImage(name, image), null) : then(null, loadError())));
    });
    if (!queued) {
      queueTask(() => then(null, loadError()));
    }
  }

  /** The error a request reports when the coordinator cannot read the database. */
  function loadError() {
    return domError('UnknownError', 'The database could not be read.');
  }

  /** Fire `error` at a finished open or delete request and let the next queued request run. */
  function failOpen(request, name, error) {
    fail(request, error);
    fireEvent(request.object, 'error', true, true);
    dequeue(name);
  }

  /** The steps of open() once the database is loaded. */
  function openLoaded(request, name, version, db, error) {
    if (error) {
      failOpen(request, name, error);
      return;
    }
    const model = db || newDatabase(name, 0);
    registry.set(name, model);
    const target = version === undefined ? Math.max(model.version, 1) : version;
    if (model.version > target) {
      failOpen(request, name, domError('VersionError', `The requested version (${target}) is less than the existing version (${model.version}).`));
      return;
    }
    const connection = newConnection(model);
    if (model.version < target) {
      upgradeWhenUnblocked(request, connection, target);
    } else {
      openSucceeded(request, connection);
    }
  }

  /** Report a successful open, unless the connection was closed during its upgrade. */
  function openSucceeded(request, connection) {
    if (connection.closePending) {
      failOpen(request, connection.name, domError('AbortError', 'The connection was closed before the open request finished.'));
      return;
    }
    succeed(request, connection.object);
    fireEvent(request.object, 'success', false, false);
    dequeue(connection.name);
  }

  /** Tell other connections a version change is coming, report blocked while they stay open, then run `then`. */
  function whenOthersClosed(db, request, oldVersion, newVersion, except, then) {
    const others = [...db.connections].filter((other) => other !== except && !other.closePending);
    for (const other of others) {
      queueTask(() => {
        if (!other.closePending) {
          fireVersionEvent(other.object, 'versionchange', oldVersion, newVersion);
        }
      });
    }
    queueTask(() => {
      const open = () => others.every((other) => other.closed);
      if (!open()) {
        fireVersionEvent(request.object, 'blocked', oldVersion, newVersion);
      }
      waitUntil(db, open, then);
    });
  }

  /** Run the upgrade once no other connection is open. */
  function upgradeWhenUnblocked(request, connection, version) {
    const db = connection.db;
    whenOthersClosed(db, request, db.version, version, connection, () => runUpgrade(request, connection, version));
  }

  /** "Run an upgrade transaction": fire upgradeneeded with the new transaction active, then wait for it to finish. */
  function runUpgrade(request, connection, version) {
    const db = connection.db;
    const oldVersion = db.version;
    const transaction = newTransaction(connection, 'versionchange', new NativeSet(db.stores.values()), 'default');
    connection.upgrade = transaction;
    setVersion(transaction, version);
    const previous = connection.version;
    connection.version = version;
    journal(transaction, () => { connection.version = previous; });
    succeed(request, connection.object);
    request.transaction = transaction;
    host.parent(request.object, transaction.object);
    transaction.finished.push(() => upgradeFinished(request, connection, transaction));
    const flags = dispatchActive(transaction, () => fireVersionEvent(request.object, 'upgradeneeded', oldVersion, version));
    if (flags & DISPATCH_THREW) {
      abortTransaction(transaction, domError('AbortError', 'An upgradeneeded handler threw.'));
    }
  }

  /** After the upgrade: success in a later task, or (aborted) close the connection and report AbortError. */
  function upgradeFinished(request, connection, transaction) {
    connection.upgrade = null;
    request.transaction = null;
    host.parent(request.object, null);
    if (!transaction.aborted) {
      queueTask(() => openSucceeded(request, connection));
      return;
    }
    connection.closePending = true;
    connectionSettled(connection);
    queueTask(() => failOpen(request, connection.name, domError('AbortError', 'The version change transaction was aborted.')));
  }

  /** The steps of deleteDatabase() once the database is loaded. */
  function deleteLoaded(request, name, db, error) {
    if (error) {
      failOpen(request, name, error);
      return;
    }
    if (!db || db.version === 0) {
      deleteSucceeded(request, name, 0);
      return;
    }
    registry.set(name, db);
    whenOthersClosed(db, request, db.version, null, null, () => {
      const version = db.version;
      registry.delete(name);
      db.version = 0;
      host.commit(jsonStringify({ database: wireName(name), ops: [{ op: 'drop' }] }));
      deleteSucceeded(request, name, version);
    });
  }

  /** Report a finished delete with the version the database had. */
  function deleteSucceeded(request, name, oldVersion) {
    succeed(request, undefined);
    fireVersionEvent(request.object, 'success', oldVersion, null);
    dequeue(name);
  }
