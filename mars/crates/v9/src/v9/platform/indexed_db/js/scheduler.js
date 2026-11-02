
  /**
   * The scheduler. IndexedDB work runs in tasks: one timer task per turn, where each queued task
   * or request is its own unit followed by a microtask checkpoint and the end-of-task
   * transaction cleanup, so promise-based callers see the same ordering a browser gives them.
   * Transactions start in creation order unless an earlier unfinished transaction with an
   * overlapping scope would conflict (either one writes); started transactions run one request
   * at a time, in request order, and commit once inactive with nothing left to run.
   */

  /** Tasks queued by the algorithms (events for aborted requests, abort, upgrade steps), run first and in order. */
  const tasks = [];
  /** Databases with transactions, in the order they first had one. */
  const busy = new NativeSet();
  /** Whether a turn is already queued. */
  let turnQueued = false;

  /** Queue an algorithm task. */
  function queueTask(task) {
    tasks.push(task);
    schedule();
  }

  /** Make sure a turn will run. */
  function schedule() {
    if (!turnQueued) {
      turnQueued = true;
      setTimeoutNative(turn, 0);
    }
  }

  /** One timer task: run up to TURN_REQUESTS units, then yield if work remains. */
  function turn() {
    turnQueued = false;
    cleanupTransactions();
    let budget = TURN_REQUESTS;
    while (budget > 0 && step()) {
      budget--;
      host.checkpoint();
      cleanupTransactions();
    }
    if (tasks.length > 0 || hasRunnableWork()) {
      schedule();
    }
  }

  /** Run one unit of work; false when nothing can run now. */
  function step() {
    if (tasks.length > 0) {
      tasks.shift()();
      return true;
    }
    for (const db of busy) {
      if (db.transactions.length === 0) {
        busy.delete(db);
      } else if (stepDatabase(db)) {
        return true;
      }
    }
    return false;
  }

  /** Whether a step could run now. */
  function hasRunnableWork() {
    for (const db of busy) {
      for (const transaction of db.transactions) {
        if (runnable(transaction)) {
          return true;
        }
      }
    }
    return false;
  }

  /** Whether a transaction has a request to run or is ready to commit. */
  function runnable(transaction) {
    if (transaction.state === 'finished' || !(transaction.started || canStart(transaction))) {
      return false;
    }
    return transaction.requests.length > 0 || transaction.state === 'inactive' || transaction.state === 'committing';
  }

  /** Advance the first transaction of a database that can make progress. */
  function stepDatabase(db) {
    for (const transaction of db.transactions) {
      if (!runnable(transaction)) {
        continue;
      }
      transaction.started = true;
      if (transaction.requests.length > 0) {
        runRequest(transaction, transaction.requests.shift());
      } else {
        commitTransaction(transaction);
      }
      return true;
    }
    return false;
  }

  /** Whether no earlier unfinished transaction of the same database conflicts with this one. */
  function canStart(transaction) {
    for (const earlier of transaction.db.transactions) {
      if (earlier === transaction) {
        return true;
      }
      if (earlier.state !== 'finished' && conflicts(earlier, transaction)) {
        return false;
      }
    }
    return true;
  }

  /** Two transactions conflict when their scopes overlap and at least one may write. */
  function conflicts(a, b) {
    if (a.mode === 'readonly' && b.mode === 'readonly') {
      return false;
    }
    if (a.mode === 'versionchange' || b.mode === 'versionchange') {
      return true;
    }
    for (const store of a.scope) {
      if (call(setHas, b.scope, store)) {
        return true;
      }
    }
    return false;
  }

  /** Execute one request's operation and fire its success or error event. */
  function runRequest(transaction, { request, operation }) {
    try {
      succeed(request, operation());
    } catch (error) {
      fail(request, error instanceof NativeDOMException ? error : domError('UnknownError', String(error)));
      fireErrorEvent(transaction, request);
      return;
    }
    fireSuccessEvent(transaction, request);
  }

  /** "Fire a success event": the transaction is active while listeners run; a listener that throws aborts it. */
  function fireSuccessEvent(transaction, request) {
    const flags = dispatchActive(transaction, () => fireEvent(request.object, 'success', false, false));
    if (flags & DISPATCH_THREW) {
      abortTransaction(transaction, domError('AbortError', 'A request success handler threw.'));
    }
  }

  /** "Fire an error event": it bubbles to the transaction and database; unless canceled, the transaction aborts with the request's error. */
  function fireErrorEvent(transaction, request) {
    const flags = dispatchActive(transaction, () => fireEvent(request.object, 'error', true, true));
    if (flags & DISPATCH_THREW) {
      abortTransaction(transaction, domError('AbortError', 'A request error handler threw.'));
    } else if (!(flags & DISPATCH_CANCELED)) {
      abortTransaction(transaction, request.error);
    }
  }

  /** Run a dispatch with an inactive transaction made active, and make it inactive again afterwards. */
  function dispatchActive(transaction, dispatch) {
    if (transaction.state === 'inactive') {
      transaction.state = 'active';
    }
    const flags = dispatch();
    if (transaction.state === 'active') {
      transaction.state = 'inactive';
    }
    schedule();
    return flags;
  }
