
  /**
   * Events and requests. Events are dispatched through `host.dispatch`, which marks them trusted,
   * propagates them along each target's event parent (request -> transaction -> database, set
   * with `host.parent`), runs a microtask checkpoint after each listener as an event loop task
   * would, and reports whether a listener canceled the event or threw.
   */

  /** IDBVersionChangeEvent: constructible by scripts, carrying the old and new versions. */
  const IDBVersionChangeEvent = makeInterface('IDBVersionChangeEvent', NativeEvent, function IDBVersionChangeEvent(type, init = undefined) {
    if (new.target === undefined) {
      throw new NativeTypeError("Failed to construct 'IDBVersionChangeEvent': Please use the 'new' operator.");
    }
    if (arguments.length < 1) {
      throw new NativeTypeError("Failed to construct 'IDBVersionChangeEvent': 1 argument required, but only 0 present.");
    }
    const dictionary = init === undefined || init === null ? {} : init;
    const event = construct(NativeEvent, [type, init], new.target);
    const oldVersion = versionMember(dictionary.oldVersion, 0);
    const newVersion = dictionary.newVersion === null ? null : versionMember(dictionary.newVersion, null);
    return bind(event, { brand: IDBVersionChangeEvent, oldVersion, newVersion });
  });

  /** An unsigned long long dictionary member; undefined takes the default. */
  function versionMember(value, fallback) {
    if (value === undefined) {
      return fallback;
    }
    const number = Math.trunc(Number(value));
    return Number.isFinite(number) ? Math.abs(number) : 0;
  }

  members(IDBVersionChangeEvent.prototype, {
    get oldVersion() { return stateOf(this, IDBVersionChangeEvent).oldVersion; },
    get newVersion() { return stateOf(this, IDBVersionChangeEvent).newVersion; },
    get dataLoss() { stateOf(this, IDBVersionChangeEvent); return 'none'; },
    get dataLossMessage() { stateOf(this, IDBVersionChangeEvent); return ''; },
  });

  /** Dispatch a new plain event; returns host.dispatch's flags. */
  function fireEvent(target, type, bubbles, cancelable) {
    return host.dispatch(target, new NativeEvent(type, { bubbles, cancelable }));
  }

  /** Dispatch a versionchange-style event carrying the old and new versions. */
  function fireVersionEvent(target, type, oldVersion, newVersion) {
    return host.dispatch(target, new IDBVersionChangeEvent(type, { oldVersion, newVersion }));
  }

  /** IDBRequest: the result or error of one asynchronous operation. */
  const IDBRequest = makeInterface('IDBRequest', NativeEventTarget);
  /** IDBOpenDBRequest: an open or delete request, which also reports blocked and upgradeneeded. */
  const IDBOpenDBRequest = makeInterface('IDBOpenDBRequest', IDBRequest);

  members(IDBRequest.prototype, {
    get result() {
      const state = stateOf(this, IDBRequest);
      if (!state.done) {
        throw stateError('The request has not finished.');
      }
      return state.result;
    },
    get error() {
      const state = stateOf(this, IDBRequest);
      if (!state.done) {
        throw stateError('The request has not finished.');
      }
      return state.error;
    },
    get source() { return stateOf(this, IDBRequest).source; },
    get transaction() {
      const transaction = stateOf(this, IDBRequest).transaction;
      return transaction ? transaction.object : null;
    },
    get readyState() { return stateOf(this, IDBRequest).done ? 'done' : 'pending'; },
  });
  eventHandlers(IDBRequest.prototype, ['success', 'error']);
  eventHandlers(IDBOpenDBRequest.prototype, ['blocked', 'upgradeneeded']);

  /** A new pending request for `source` in `transaction` (null for open and delete requests). */
  function newRequest(source, transaction, open) {
    const state = { brand: IDBRequest, done: false, result: undefined, error: null, source, transaction, open };
    const object = eventTargetInstance(open ? IDBOpenDBRequest : IDBRequest, state);
    state.object = object;
    host.parent(object, transaction ? transaction.object : null);
    return state;
  }

  /** Settle a request with a result. */
  function succeed(request, result) {
    request.done = true;
    request.result = result;
    request.error = null;
  }

  /** Settle a request with an error; its result becomes undefined. */
  function fail(request, error) {
    request.done = true;
    request.result = undefined;
    request.error = error;
  }
