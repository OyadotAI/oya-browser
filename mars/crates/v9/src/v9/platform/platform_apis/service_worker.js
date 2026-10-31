  /**
   * Service workers as pages see them: ServiceWorkerContainer, ServiceWorkerRegistration,
   * ServiceWorker and NavigationPreloadManager. The coordinator runs the Register, Update,
   * Install and Activate jobs and the workers themselves; this side keeps one object per
   * registration and worker id so identity holds, and applies the state changes, controller
   * changes and messages the coordinator delivers on the "sw" topic.
   */
  const swState = fromJson(host.state() || 'null') || {};
  const workerProto = prototypeOf('ServiceWorker', global.EventTarget && global.EventTarget.prototype);
  const registrationProto = prototypeOf('ServiceWorkerRegistration', global.EventTarget && global.EventTarget.prototype);
  const containerProto = prototypeOf('ServiceWorkerContainer', global.EventTarget && global.EventTarget.prototype);
  const preloadProto = prototypeOf('NavigationPreloadManager');
  const workerRecords = new WeakMap();
  const registrationRecords = new WeakMap();
  const workerObjects = new Map();
  const registrationObjects = new Map();
  const ExtendableMessageEvent = eventClass('ExtendableMessageEvent', { data: null, origin: '', lastEventId: '', source: null, ports: freeze([]) });

  /** The ServiceWorker object for a coordinator worker record, updated in place. */
  function workerObject(record) {
    if (!record) return null;
    let worker = workerObjects.get(record.id);
    if (!worker) {
      worker = create(workerProto);
      workerObjects.set(record.id, worker);
      workerRecords.set(worker, { ...record });
    }
    return worker;
  }

  /** Make a worker object reflect a newer record, firing statechange when its state moved. */
  function updateWorker(record) {
    const worker = workerObject(record);
    if (!worker) return;
    const known = workerRecords.get(worker);
    if (known.state !== record.state) {
      known.state = record.state;
      worker.dispatchEvent(new EventCtor('statechange'));
    }
  }

  /** The ServiceWorkerRegistration object for a record, its worker slots refreshed. */
  function registrationObject(record) {
    if (!record) return undefined;
    let registration = registrationObjects.get(record.id);
    if (!registration) {
      registration = create(registrationProto);
      registrationObjects.set(record.id, registration);
      const preload = create(preloadProto);
      registrationRecords.set(registration, { record, preload });
      registrationRecords.set(preload, { record, preload, owner: registration });
    }
    registrationRecords.get(registration).record = record;
    registrationRecords.get(registrationRecords.get(registration).preload).record = record;
    for (const slot of ['installing', 'waiting', 'active']) updateWorker(record[slot]);
    return registration;
  }

  /** A registration's current record, or a TypeError for another receiver. */
  function registrationRecord(registration) {
    const entry = registrationRecords.get(registration);
    if (!entry) throw new TypeError('Illegal invocation');
    return entry.record;
  }

  getter(workerProto, 'scriptURL', (worker) => (workerRecords.get(worker) || {}).scriptURL);
  getter(workerProto, 'state', (worker) => (workerRecords.get(worker) || {}).state);
  eventHandler(workerProto, 'statechange');
  eventHandler(workerProto, 'error');
  methods(workerProto, {
    postMessage(message, options) {
      const record = workerRecords.get(this);
      if (!record) throw new TypeError('Illegal invocation');
      if (record.state === 'redundant') return;
      const transfer = isArray(options) ? options : options && options.transfer;
      if (transfer && transfer.length) throw domError('DataCloneError', 'Transferring objects to a service worker is not supported.');
      call('sw.postMessage', { worker: record.id, data: encodeClone(message) }).catch(reportError);
    },
  });

  for (const slot of ['installing', 'waiting', 'active']) {
    getter(registrationProto, slot, (registration) => workerObject(registrationRecord(registration)[slot]));
  }
  getter(registrationProto, 'scope', (registration) => registrationRecord(registration).scope);
  getter(registrationProto, 'updateViaCache', (registration) => registrationRecord(registration).updateViaCache);
  getter(registrationProto, 'navigationPreload', (registration) => registrationRecords.get(registration).preload);
  eventHandler(registrationProto, 'updatefound');
  methods(registrationProto, {
    async update() {
      return registrationObject(await call('sw.update', { scope: registrationRecord(this).scope }));
    },
    async unregister() {
      return call('sw.unregister', { scope: registrationRecord(this).scope });
    },
    async showNotification() {
      throw new TypeError("Failed to execute 'showNotification' on 'ServiceWorkerRegistration': No notification permission has been granted for this origin.");
    },
    async getNotifications() { return []; },
  });

  methods(preloadProto, {
    async enable() { await preloadCall(this, { enabled: true }); },
    async disable() { await preloadCall(this, { enabled: false }); },
    async setHeaderValue(value) { await preloadCall(this, { headerValue: String(value) }); },
    async getState() {
      const record = registrationRecords.get(this).record;
      return { enabled: record.preload.enabled, headerValue: record.preload.headerValue };
    },
  });

  /** Change a registration's navigation preload state; requires an active worker. */
  async function preloadCall(manager, change) {
    const entry = registrationRecords.get(manager);
    if (!entry) throw new TypeError('Illegal invocation');
    if ('enabled' in change && !entry.record.active) throw domError('InvalidStateError', 'The registration does not have an active worker.');
    registrationObject(await call('sw.preload', { scope: entry.record.scope, ...change }));
  }

  /** Apply one coordinator "sw" event to the objects this context made. */
  function serviceWorkerEvent(event) {
    if (event.kind === 'state') {
      if (event.registration) registrationObject(event.registration);
      if (workerObjects.has(event.worker.id)) updateWorker(event.worker);
    } else if (event.kind === 'updatefound') {
      const registration = registrationObject(event.registration);
      registration.dispatchEvent(new EventCtor('updatefound'));
    } else if (event.kind === 'registration') {
      registrationObject(event.registration);
    } else if (event.kind === 'controller') {
      setController(event.worker);
    } else if (event.kind === 'message') {
      deliverClientMessage(event);
    }
  }

  /** The current controller and whether startMessages() let messages through. */
  let controller = workerObject(swState.controller);
  let messagesStarted = false;
  const messageQueue = [];
  let containerObject = null;

  /** Make `record` the controller and fire controllerchange. */
  function setController(record) {
    controller = workerObject(record);
    if (containerObject) containerObject.dispatchEvent(new EventCtor('controllerchange'));
  }

  /** A message from a service worker: queued until startMessages, then a MessageEvent. */
  function deliverClientMessage(event) {
    messageQueue.push(event);
    // HTML enables the client message queue when the document finishes parsing.
    if (!messagesStarted && global.document && global.document.readyState !== 'loading') messagesStarted = true;
    if (messagesStarted) flushMessages();
  }

  /** Dispatch every queued client message at the container. */
  function flushMessages() {
    while (messageQueue.length && containerObject) {
      const event = messageQueue.shift();
      const message = makeEvent('MessageEvent', 'message', {}, {
        data: decodeClone(event.data), origin: event.origin, lastEventId: '', source: workerObject(event.source), ports: freeze([]),
      });
      containerObject.dispatchEvent(message);
    }
  }

  /** Watch this context's service worker events once. */
  let swWatching = null;
  function watchServiceWorkers() {
    if (!swWatching) swWatching = subscribe('sw', serviceWorkerEvent);
    return swWatching;
  }

  /** Whether a URL string is http(s) after resolution. */
  function httpUrl(url) {
    return url.protocol === 'http:' || url.protocol === 'https:';
  }

  /** Validate register()'s script URL and scope as "Start Register" does. */
  function registerArguments(scriptURL, options) {
    const base = global.document ? global.document.baseURI : global.location.href;
    let script;
    try { script = new global.URL(String(scriptURL), base); } catch (error) {
      throw new TypeError(`Failed to register a ServiceWorker: The URL protocol of the script ('${String(scriptURL)}') is not supported.`);
    }
    if (!httpUrl(script)) throw new TypeError(`Failed to register a ServiceWorker: The URL protocol of the script ('${script.href}') is not supported.`);
    if (/%2f|%5c/i.test(script.pathname)) throw new TypeError("Failed to register a ServiceWorker: The provided scriptURL includes a disallowed escape character.");
    const scope = options.scope === undefined ? new global.URL('./', script) : new global.URL(String(options.scope), base);
    if (!httpUrl(scope)) throw new TypeError(`Failed to register a ServiceWorker: The URL protocol of the scope ('${scope.href}') is not supported.`);
    if (/%2f|%5c/i.test(scope.pathname)) throw new TypeError("Failed to register a ServiceWorker: The provided scope includes a disallowed escape character.");
    const type = options.type === undefined ? 'classic' : String(options.type);
    if (type !== 'classic' && type !== 'module') throw new TypeError(`Failed to execute 'register' on 'ServiceWorkerContainer': The provided value '${type}' is not a valid enum value of type WorkerType.`);
    const updateViaCache = options.updateViaCache === undefined ? 'imports' : String(options.updateViaCache);
    if (!['imports', 'all', 'none'].includes(updateViaCache)) {
      throw new TypeError(`Failed to execute 'register' on 'ServiceWorkerContainer': The provided value '${updateViaCache}' is not a valid enum value of type ServiceWorkerUpdateViaCache.`);
    }
    script.hash = '';
    scope.hash = '';
    return { scriptURL: script.href, scope: scope.href, type, updateViaCache };
  }

  getter(containerProto, 'controller', () => { watchServiceWorkers(); return controller; });
  const addContainerListener = global.EventTarget && global.EventTarget.prototype.addEventListener;
  method(containerProto, 'addEventListener', function addEventListener(type, listener, options) {
    watchServiceWorkers();
    return addContainerListener.call(this, type, listener, options);
  });
  let readyPromise = null;
  getter(containerProto, 'ready', () => {
    if (!readyPromise) readyPromise = watchServiceWorkers().then(() => call('sw.ready', {})).then(registrationObject);
    return readyPromise;
  });
  eventHandler(containerProto, 'controllerchange');
  eventHandler(containerProto, 'messageerror');
  defineProperty(containerProto, 'onmessage', {
    enumerable: true,
    configurable: true,
    get: { onmessage() { const slots = handlers.get(this); return (slots && slots.message && slots.message.value) || null; } }.onmessage,
    set: { onmessage(value) { setHandler(this, 'message', value); this.startMessages(); } }.onmessage,
  });
  methods(containerProto, {
    async register(scriptURL, options) {
      if (!secure()) throw domError('SecurityError', 'Failed to register a ServiceWorker: The document is in an invalid state.');
      const args = registerArguments(scriptURL, options || {});
      await watchServiceWorkers();
      return registrationObject(await call('sw.register', args));
    },
    async getRegistration(clientURL) {
      const base = global.document ? global.document.baseURI : global.location.href;
      const url = new global.URL(clientURL === undefined ? '' : String(clientURL), base);
      if (url.origin !== global.location.origin) throw domError('SecurityError', "Failed to execute 'getRegistration' on 'ServiceWorkerContainer': The origin of the provided documentURL does not match the current origin.");
      await watchServiceWorkers();
      return registrationObject(await call('sw.getRegistration', { url: url.href }));
    },
    async getRegistrations() {
      await watchServiceWorkers();
      return (await call('sw.getRegistrations', {})).map(registrationObject);
    },
    startMessages() {
      messagesStarted = true;
      watchServiceWorkers();
      flushMessages();
    },
  });

  if (global.navigator && (CONTEXT === 'window' || CONTEXT === 'frame') && secure()) {
    containerObject = global.navigator.serviceWorker || create(containerProto);
    if (controller) watchServiceWorkers();
  }
