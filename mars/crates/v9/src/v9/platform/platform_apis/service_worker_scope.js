  /**
   * ServiceWorkerGlobalScope, for a worker the coordinator started as a service worker:
   * `registration`, `serviceWorker`, `clients`, `skipWaiting()` and the install, activate,
   * fetch and message events with ExtendableEvent.waitUntil and FetchEvent.respondWith. The
   * scope keeps one `sw.next` request parked in the coordinator, which answers it with the next
   * functional event; each event's outcome goes back with `sw.done`.
   */
  if (CONTEXT === 'service') {
    const ExtendableEvent = eventClass('ExtendableEvent', {});
    const FetchEvent = eventClass('FetchEvent', { request: null, clientId: '', resultingClientId: '', replacesClientId: '', handled: null, preloadResponse: null });
    const lifetimes = new WeakMap();
    const responses = new WeakMap();

    /** Give an event the waitUntil() its lifetime promises collect in. */
    function extendable(event) {
      const state = { promises: [], dispatching: true, pending: 0 };
      lifetimes.set(event, state);
      defineProperty(event, 'waitUntil', {
        value: function waitUntil(promise) {
          if (!state.dispatching && state.pending === 0) {
            throw domError('InvalidStateError', "Failed to execute 'waitUntil' on 'ExtendableEvent': The event handler is already finished and no extend lifetime promises are outstanding.");
          }
          state.pending += 1;
          const settled = Promise.resolve(promise).finally(() => { state.pending -= 1; });
          state.promises.push(settled);
        },
        configurable: true,
      });
      return state;
    }

    /** Settle once every lifetime promise has, including ones added while waiting. */
    async function lifetime(state) {
      state.dispatching = false;
      let seen = 0;
      let failed = false;
      while (seen < state.promises.length) {
        const batch = state.promises.slice(seen);
        seen = state.promises.length;
        const results = await Promise.allSettled(batch);
        failed = failed || results.some((result) => result.status === 'rejected');
      }
      return !failed;
    }

    /** Whether the script registered a fetch handler, which the coordinator records at install
    so it can skip workers that would only fall back to the network. */
    let fetchHandlers = false;
    const addScopeListener = global.addEventListener;
    method(global, 'addEventListener', function addEventListener(type, listener, options) {
      if (String(type) === 'fetch' && listener) fetchHandlers = true;
      return addScopeListener.call(this, type, listener, options);
    });

    /** Run install or activate: dispatch, wait for waitUntil, report success. */
    async function lifecycleEvent(record) {
      const event = new ExtendableEvent(record.kind);
      const state = extendable(event);
      let ok = true;
      try { global.dispatchEvent(event); } catch (error) { ok = false; reportError(error); }
      ok = (await lifetime(state)) && ok;
      return { answer: { ok, fetch: fetchHandlers || typeof global.onfetch === 'function' } };
    }

    /** The Request a fetch event carries, rebuilt from the coordinator's record. */
    function fetchRequest(record) {
      const init = { method: record.method, headers: record.headers, credentials: record.credentials || 'include', redirect: record.redirect || 'follow' };
      if (record.body !== undefined && record.body !== null && record.method !== 'GET' && record.method !== 'HEAD') init.body = record.body;
      const request = new RequestCtor(record.url, init);
      for (const key of ['mode', 'destination']) {
        if (record[key]) defineProperty(request, key, { value: record[key], enumerable: true, configurable: true });
      }
      return request;
    }

    /** The Response navigation preload fetched, rebuilt from the coordinator's record. */
    function preloadResponse(record) {
      const response = new ResponseCtor(record.body, { status: record.status, headers: record.headers });
      restoreIdentity(response, record.url, 'basic');
      return response;
    }

    /** Run a fetch event: the response respondWith gave, a fallback to the network, or an error. */
    async function fetchEvent(record) {
      let handledResolve;
      const handled = new Promise((resolve) => { handledResolve = resolve; });
      handled.catch(() => {});
      const event = new FetchEvent('fetch', {
        cancelable: true,
        request: fetchRequest(record),
        clientId: record.clientId || '',
        resultingClientId: record.resultingClientId || '',
        handled,
        preloadResponse: Promise.resolve(record.preload ? preloadResponse(record.preload) : undefined),
      });
      const state = extendable(event);
      const slot = { called: false, promise: null };
      responses.set(event, slot);
      defineProperty(event, 'respondWith', {
        value: function respondWith(response) {
          if (!state.dispatching) throw domError('InvalidStateError', "Failed to execute 'respondWith' on 'FetchEvent': The event handler is already finished.");
          if (slot.called) throw domError('InvalidStateError', "Failed to execute 'respondWith' on 'FetchEvent': The event has already been responded to.");
          slot.called = true;
          event.stopImmediatePropagation();
          slot.promise = Promise.resolve(response);
          state.promises.push(slot.promise.then(() => {}, () => {}));
        },
        configurable: true,
      });
      try { global.dispatchEvent(event); } catch (error) { reportError(error); }
      state.dispatching = false;
      if (!slot.called) {
        handledResolve();
        lifetime(state);
        return { answer: { fallback: !event.defaultPrevented, error: event.defaultPrevented } };
      }
      try {
        const response = await slot.promise;
        if (!(response instanceof ResponseCtor) || response.bodyUsed) throw new TypeError('The FetchEvent was responded to with a value that is not a usable Response.');
        const read = await responseRecord(response);
        handledResolve();
        lifetime(state);
        return { answer: { response: read.record }, bytes: read.bytes };
      } catch (error) {
        reportError(error);
        handledResolve();
        return { answer: { error: true } };
      }
    }

    /** Run a message event from a client or another worker. */
    async function messageEvent(record) {
      const event = new ExtendableMessageEvent('message', { data: decodeClone(record.data), origin: record.origin, source: record.source ? clientObject(record.source) : null });
      const state = extendable(event);
      try { global.dispatchEvent(event); } catch (error) { reportError(error); }
      await lifetime(state);
      return { ok: true };
    }

    /** One functional event to its runner by kind. */
    const runners = { install: lifecycleEvent, activate: lifecycleEvent, fetch: fetchEvent, message: messageEvent };

    /** Process one event and report its outcome; the next one is already being waited for. */
    async function runEvent(record) {
      const runner = Object.prototype.hasOwnProperty.call(runners, record.kind) ? runners[record.kind] : null;
      if (!runner) return;
      let outcome;
      try { outcome = await runner(record); } catch (error) { reportError(error); outcome = { ok: false, answer: { error: true } }; }
      const answer = outcome.answer || { ok: outcome.ok };
      call('sw.done', { event: record.id, ...answer }, outcome.bytes).catch(() => {});
    }

    /** Keep one `sw.next` parked; each answer is a batch of events. */
    function nextEvents() {
      call('sw.next', {}).then((events) => {
        for (const record of events || []) runEvent(record);
        nextEvents();
      }, () => {});
    }

    const clientProto = prototypeOf('Client');
    const windowClientProto = prototypeOf('WindowClient', clientProto);
    const clientRecords = new WeakMap();
    /** A Client (a WindowClient for windows) over a coordinator client record. */
    function clientObject(record) {
      const client = create(record.type === 'window' ? windowClientProto : clientProto);
      clientRecords.set(client, record);
      return client;
    }
    for (const name of ['id', 'url', 'type', 'frameType', 'focused', 'visibilityState']) {
      getter(clientProto, name, (client) => (clientRecords.get(client) || {})[name]);
    }
    methods(clientProto, {
      postMessage(message, options) {
        const record = clientRecords.get(this);
        if (!record) throw new TypeError('Illegal invocation');
        const transfer = isArray(options) ? options : options && options.transfer;
        if (transfer && transfer.length) throw domError('DataCloneError', 'Transferring objects to a client is not supported.');
        call('sw.clientPost', { client: record.id, data: encodeClone(message) }).catch(reportError);
      },
    });
    methods(windowClientProto, {
      async focus() { throw domError('InvalidAccessError', 'Not allowed to focus a window.'); },
      async navigate(url) {
        const record = clientRecords.get(this);
        const target = new global.URL(String(url), record.url);
        return clientObject(await call('sw.navigate', { client: record.id, url: target.href }));
      },
    });
    const clientsProto = prototypeOf('Clients');
    methods(clientsProto, {
      async get(id) {
        const record = await call('sw.clients', { id: String(id) });
        return record.length ? clientObject(record[0]) : undefined;
      },
      async matchAll(options) {
        const value = options || {};
        const records = await call('sw.clients', { includeUncontrolled: Boolean(value.includeUncontrolled), type: value.type === undefined ? 'window' : String(value.type) });
        return records.map(clientObject);
      },
      async openWindow() { throw domError('InvalidAccessError', 'Not allowed to open a window.'); },
      async claim() { await call('sw.claim', {}); },
    });

    const scopeState = swState;
    const clients = create(clientsProto);
    const ownRegistration = registrationObject(scopeState.registration);
    const scopeMembers = {
      registration: () => ownRegistration,
      serviceWorker: () => workerObject(scopeState.worker),
      clients: () => clients,
    };
    for (const name of Object.keys(scopeMembers)) {
      defineProperty(global, name, { get: { [name]() { return scopeMembers[name](); } }[name], enumerable: true, configurable: true });
    }
    method(global, 'skipWaiting', function skipWaiting() { return call('sw.skipWaiting', {}).then(() => undefined); });
    for (const type of ['install', 'activate', 'fetch', 'message', 'messageerror']) eventHandler(global, type);
    defineProperty(global, Symbol.toStringTag, { value: 'ServiceWorkerGlobalScope', configurable: true });
    // A service worker has no owner to post to and is not closed by script.
    for (const holder of [global, Object.getPrototypeOf(global)]) {
      delete holder.postMessage;
      delete holder.close;
    }
    subscribe('sw', serviceWorkerEvent);
    nextEvents();
  }
