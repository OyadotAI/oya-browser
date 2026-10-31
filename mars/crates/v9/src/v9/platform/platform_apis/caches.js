  /**
   * Cache Storage: `caches` (CacheStorage) and Cache, in windows, workers and service workers.
   * Entries live in the coordinator per storage partition and persist with the profile; request
   * matching (method, URL without fragment, ignoreSearch/ignoreMethod/ignoreVary and Vary) is
   * the coordinator's. Bodies cross as bytes; one answer carries several bodies end to end with
   * their lengths.
   */
  const cacheProto = prototypeOf('Cache');
  const cacheStorageProto = prototypeOf('CacheStorage');
  const cacheIds = new WeakMap();
  const RequestCtor = global.Request;
  const ResponseCtor = global.Response;
  /** Statuses `new Response` accepts; others (an opaque 0) are restored on the instance. */
  const MIN_STATUS = 200;
  const MAX_STATUS = 599;

  /** A Request from a RequestInfo (string URLs resolve against this context's base). */
  function toRequest(input) {
    return input instanceof RequestCtor ? input : new RequestCtor(input);
  }

  /** The serializable request fields matching needs. */
  function requestRecord(request) {
    return { url: request.url, method: request.method, headers: [...request.headers] };
  }

  /** CacheQueryOptions as the coordinator reads them. */
  function queryOptions(options) {
    const value = options || {};
    return { ignoreSearch: Boolean(value.ignoreSearch), ignoreMethod: Boolean(value.ignoreMethod), ignoreVary: Boolean(value.ignoreVary) };
  }

  /** Throw the TypeError Cache.put/add give a request that is not an http(s) GET. */
  function checkCacheable(request, what) {
    if (!/^https?:$/.test(new global.URL(request.url).protocol)) {
      throw new TypeError(`Failed to execute '${what}' on 'Cache': Request scheme '${new global.URL(request.url).protocol.slice(0, -1)}' is unsupported`);
    }
    if (request.method !== 'GET') throw new TypeError(`Failed to execute '${what}' on 'Cache': Request method '${request.method}' is unsupported`);
  }

  /** Throw the TypeError a response the cache refuses gets. */
  function checkResponse(response, what) {
    if (response.status === 206) throw new TypeError(`Failed to execute '${what}' on 'Cache': Partial response (status code 206) is unsupported`);
    const vary = response.headers.get('vary');
    if (vary && vary.split(',').some((field) => field.trim() === '*')) {
      throw new TypeError(`Failed to execute '${what}' on 'Cache': Vary header contains *`);
    }
    if (response.bodyUsed) throw new TypeError(`Failed to execute '${what}' on 'Cache': Response body is already used`);
  }

  /** Read one response into the record and bytes `cache.put` sends. */
  async function responseRecord(response) {
    const bytes = new Uint8Array(await response.arrayBuffer());
    return {
      record: { url: response.url, status: response.status, statusText: response.statusText, type: response.type, headers: [...response.headers], length: bytes.length },
      bytes,
    };
  }

  /** Rebuild Responses from coordinator records and their concatenated bodies. */
  function responsesFrom(records, buffer) {
    const all = buffer ? new Uint8Array(buffer) : new Uint8Array(0);
    let offset = 0;
    return records.map((record) => {
      const body = all.slice(offset, offset + record.length);
      offset += record.length;
      const ordinary = record.status >= MIN_STATUS && record.status <= MAX_STATUS;
      const nullBody = [101, 204, 205, 304].includes(record.status);
      const response = new ResponseCtor(nullBody || record.type === 'opaque' ? null : body, {
        status: ordinary ? record.status : MIN_STATUS,
        statusText: record.statusText,
        headers: record.type === 'opaque' ? [] : record.headers,
      });
      defineProperty(response, 'url', { value: record.url, enumerable: true, configurable: true });
      defineProperty(response, 'type', { value: record.type || 'default', enumerable: true, configurable: true });
      if (!ordinary) {
        defineProperty(response, 'status', { value: record.status, enumerable: true, configurable: true });
        defineProperty(response, 'ok', { value: false, enumerable: true, configurable: true });
      }
      return response;
    });
  }

  /** A Cache object for coordinator cache `id`. */
  function cacheObject(id) {
    const cache = create(cacheProto);
    cacheIds.set(cache, id);
    return cache;
  }

  /** The coordinator id of a Cache, or a TypeError for another receiver. */
  function cacheId(cache) {
    const id = cacheIds.get(cache);
    if (id === undefined) throw new TypeError('Illegal invocation');
    return id;
  }

  /** Store fetched or given pairs in one batch, as Cache's batch operation does. */
  async function putAll(cache, pairs) {
    const records = [];
    const bodies = [];
    for (const [request, response] of pairs) {
      const read = await responseRecord(response);
      records.push({ request: requestRecord(request), response: read.record });
      bodies.push(read.bytes);
    }
    const total = bodies.reduce((sum, bytes) => sum + bytes.length, 0);
    const joined = new Uint8Array(total);
    let offset = 0;
    for (const bytes of bodies) { joined.set(bytes, offset); offset += bytes.length; }
    await call('cache.put', { cache: cacheId(cache), entries: records }, joined);
  }

  methods(cacheProto, {
    async match(request, options) {
      const found = await this.matchAll(request, options);
      return found.length ? found[0] : undefined;
    },
    async matchAll(request, options) {
      const id = cacheId(this);
      const query = request === undefined ? null : requestRecord(toRequest(request));
      if (query && query.method !== 'GET' && !(options && options.ignoreMethod)) return [];
      const answer = await callBytes('cache.match', { cache: id, request: query, options: queryOptions(options) });
      return responsesFrom(answer.ok, answer.bytes);
    },
    async add(request) {
      return this.addAll([request]);
    },
    async addAll(requests) {
      cacheId(this);
      const list = Array.from(requests, toRequest);
      for (const request of list) checkCacheable(request, 'addAll');
      const responses = await Promise.all(list.map((request) => global.fetch(request)));
      responses.forEach((response) => {
        if (!response.ok) throw new TypeError("Failed to execute 'addAll' on 'Cache': Request failed");
        checkResponse(response, 'addAll');
      });
      await putAll(this, list.map((request, index) => [request, responses[index]]));
    },
    async put(request, response) {
      cacheId(this);
      const target = toRequest(request);
      checkCacheable(target, 'put');
      if (!(response instanceof ResponseCtor)) throw new TypeError("Failed to execute 'put' on 'Cache': parameter 2 is not of type 'Response'.");
      checkResponse(response, 'put');
      await putAll(this, [[target, response]]);
    },
    async delete(request, options) {
      const query = requestRecord(toRequest(request));
      if (query.method !== 'GET' && !(options && options.ignoreMethod)) return false;
      return call('cache.delete', { cache: cacheId(this), request: query, options: queryOptions(options) });
    },
    async keys(request, options) {
      const query = request === undefined ? null : requestRecord(toRequest(request));
      const records = await call('cache.keys', { cache: cacheId(this), request: query, options: queryOptions(options) });
      return records.map((record) => new RequestCtor(record.url, { method: record.method, headers: record.headers }));
    },
  });

  methods(cacheStorageProto, {
    async open(name) {
      return cacheObject(await call('caches.open', { name: String(name) }));
    },
    has(name) { return call('caches.has', { name: String(name) }); },
    delete(name) { return call('caches.delete', { name: String(name) }); },
    keys() { return call('caches.keys', {}); },
    async match(request, options) {
      const query = requestRecord(toRequest(request));
      const name = options && options.cacheName !== undefined ? String(options.cacheName) : null;
      if (query.method !== 'GET' && !(options && options.ignoreMethod)) return undefined;
      const answer = await callBytes('caches.match', { name, request: query, options: queryOptions(options) });
      const found = responsesFrom(answer.ok, answer.bytes);
      return found.length ? found[0] : undefined;
    },
  });

  if (secure()) {
    const storage = create(cacheStorageProto);
    defineProperty(global, 'caches', { get: { caches() { return storage; } }.caches, enumerable: true, configurable: true });
    for (const [name, proto] of [['Cache', cacheProto], ['CacheStorage', cacheStorageProto]]) {
      if (!global[name]) {
        const ctor = { [name]: function () { throw new TypeError('Illegal constructor'); } }[name];
        ctor.prototype = proto;
        defineGlobal(name, ctor);
      }
    }
  }
