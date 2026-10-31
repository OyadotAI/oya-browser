  /**
   * Web Locks. The lock manager is the coordinator's, shared by every context of a storage key
   * (tabs, frames, workers); a request parks there until it is granted, and the callback's
   * settled result releases it. A stolen lock rejects its holder's promise with AbortError.
   */
  const lockProto = prototypeOf('Lock');
  const lockInfo = new WeakMap();
  getter(lockProto, 'name', (lock) => (lockInfo.get(lock) || {}).name);
  getter(lockProto, 'mode', (lock) => (lockInfo.get(lock) || {}).mode);
  /** Held locks by request id: how to reject their request() promise when stolen. */
  const heldLocks = new Map();
  let nextLock = 0;

  /** The validated options of one request() call; throws the DOMException the spec names. */
  function lockOptions(name, options) {
    const mode = options.mode === undefined ? 'exclusive' : String(options.mode);
    if (mode !== 'exclusive' && mode !== 'shared') {
      throw new TypeError(`Failed to execute 'request' on 'LockManager': The provided value '${mode}' is not a valid enum value of type LockMode.`);
    }
    const steal = Boolean(options.steal);
    const ifAvailable = Boolean(options.ifAvailable);
    if (name.startsWith('-')) throw domError('NotSupportedError', "Names cannot start with '-'.");
    if (steal && ifAvailable) throw domError('NotSupportedError', "The 'steal' and 'ifAvailable' options cannot be used together.");
    if (steal && mode !== 'exclusive') throw domError('NotSupportedError', "The 'steal' option may only be used with 'exclusive' locks.");
    if (options.signal && (steal || ifAvailable)) {
      throw domError('NotSupportedError', "The 'signal' option may not be used with 'steal' or 'ifAvailable'.");
    }
    return { mode, steal, ifAvailable, signal: options.signal || null };
  }

  /** Run the callback with the granted lock (or null) and release it once its result settles. */
  async function runGranted(id, name, options, callback, granted) {
    let lock = null;
    if (granted) {
      lock = create(lockProto);
      lockInfo.set(lock, { name, mode: options.mode });
    }
    let stolen;
    const theft = new Promise((resolve, reject) => { stolen = reject; });
    theft.catch(() => {});
    if (granted) heldLocks.set(id, stolen);
    try {
      return await Promise.race([Promise.resolve().then(() => callback(lock)), theft]);
    } finally {
      if (granted && heldLocks.delete(id)) call('locks.release', { id }).catch(() => {});
    }
  }

  const lockManagerProto = prototypeOf('LockManager');
  methods(lockManagerProto, {
    async request(name, options, callback) {
      if (arguments.length < 2) throw new TypeError("Failed to execute 'request' on 'LockManager': 2 arguments required.");
      if (typeof options === 'function' && callback === undefined) { callback = options; options = {}; }
      if (typeof callback !== 'function') throw new TypeError("Failed to execute 'request' on 'LockManager': The callback provided as parameter 3 is not a function.");
      const lockName = String(name);
      const parsed = lockOptions(lockName, options || {});
      const aborted = abortReason(parsed.signal);
      if (aborted) throw aborted;
      await watchSteals();
      nextLock += 1;
      const id = nextLock;
      const onAbort = () => { call('locks.abort', { id }).catch(() => {}); };
      if (parsed.signal) parsed.signal.addEventListener('abort', onAbort);
      const answer = await call('locks.request', { id, name: lockName, mode: parsed.mode, ifAvailable: parsed.ifAvailable, steal: parsed.steal });
      if (parsed.signal) parsed.signal.removeEventListener('abort', onAbort);
      if (answer.aborted) throw abortReason(parsed.signal) || domError('AbortError', 'The request was aborted.');
      return runGranted(id, lockName, parsed, callback, answer.granted);
    },
    query() {
      return call('locks.query', {});
    },
  });
  /** A steal elsewhere rejects the holder's request() promise; subscribed on first use. */
  function watchSteals() {
    return subscribe('locks', (event) => {
      const reject = heldLocks.get(event.stolen);
      if (reject) {
        heldLocks.delete(event.stolen);
        reject(domError('AbortError', 'The lock was stolen by another request.'));
      }
    });
  }
