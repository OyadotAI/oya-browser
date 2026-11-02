  /**
   * Cookie Store API over the coordinator's real cookie jar: the same cookies document.cookie
   * sees (never HttpOnly ones), set as the spec says (Secure, SameSite=Strict by default), and
   * `change` events when the jar's script-visible cookies for this URL change.
   */
  /** The longest name plus value the spec admits, in bytes. */
  const COOKIE_PAIR_BYTES = 4096;

  /** One CookieListItem as the spec shapes it. */
  function cookieItem(cookie) {
    return {
      name: cookie.name,
      value: cookie.value,
      domain: cookie.domain,
      path: cookie.path,
      expires: cookie.expires,
      secure: cookie.secure,
      sameSite: cookie.sameSite,
      partitioned: cookie.partitioned,
    };
  }

  /** The name and URL a get/getAll call asks for: a string name or a CookieStoreGetOptions. */
  function cookieQuery(what, init) {
    const options = typeof init === 'object' && init !== null ? init : init === undefined ? {} : { name: String(init) };
    const query = {};
    if (options.name !== undefined) query.name = String(options.name);
    if (options.url !== undefined) {
      const url = new global.URL(String(options.url), global.location.href);
      if (CONTEXT === 'window' || CONTEXT === 'frame') {
        if (url.href.split('#')[0] !== global.location.href.split('#')[0]) {
          throw new TypeError(`Failed to execute '${what}' on 'CookieStore': URL must match the document URL`);
        }
      } else if (url.origin !== global.location.origin) {
        throw new TypeError(`Failed to execute '${what}' on 'CookieStore': URL must be same-origin`);
      }
      query.url = url.href;
    }
    return query;
  }

  /** The cookie a set() call describes, validated as the spec's "set a cookie" does. */
  function cookieInit(name, value) {
    const init = typeof name === 'object' && name !== null ? { ...name } : { name: String(name), value: String(value) };
    init.name = String(init.name === undefined ? '' : init.name);
    init.value = String(init.value === undefined ? '' : init.value);
    if (/[\u0000-\u001f\u007f;]/.test(init.name + init.value) || init.name.includes('=')) {
      throw new TypeError("Failed to execute 'set' on 'CookieStore': Cookie name or value contains invalid characters.");
    }
    if (init.name === '' && init.value.includes('=')) {
      throw new TypeError("Failed to execute 'set' on 'CookieStore': Cookie value cannot contain '=' if the name is empty.");
    }
    if (init.name.length + init.value.length > COOKIE_PAIR_BYTES) {
      throw new TypeError("Failed to execute 'set' on 'CookieStore': Cookie name and value are too long.");
    }
    if (init.path !== undefined && init.path !== null && !String(init.path).startsWith('/')) {
      throw new TypeError("Failed to execute 'set' on 'CookieStore': Cookie path must start with '/'.");
    }
    const sameSite = init.sameSite === undefined ? 'strict' : String(init.sameSite);
    if (!['strict', 'lax', 'none'].includes(sameSite)) {
      throw new TypeError(`Failed to execute 'set' on 'CookieStore': The provided value '${sameSite}' is not a valid enum value of type CookieSameSite.`);
    }
    return {
      name: init.name,
      value: init.value,
      expires: init.expires === undefined || init.expires === null ? null : Number(init.expires),
      domain: init.domain === undefined || init.domain === null ? null : String(init.domain),
      path: init.path === undefined || init.path === null ? '/' : String(init.path),
      sameSite,
      partitioned: Boolean(init.partitioned),
    };
  }

  const cookieStoreProto = prototypeOf('CookieStore', global.EventTarget && global.EventTarget.prototype);
  methods(cookieStoreProto, {
    async get(init) {
      if (arguments.length === 0 || (typeof init === 'object' && init !== null && init.name === undefined && init.url === undefined)) {
        if (arguments.length === 0) throw new TypeError("Failed to execute 'get' on 'CookieStore': 1 argument required, but only 0 present.");
        throw new TypeError("Failed to execute 'get' on 'CookieStore': Options must not be empty");
      }
      const list = await call('cookies.get', cookieQuery('get', init));
      return list.length ? cookieItem(list[0]) : null;
    },
    async getAll(init) {
      const list = await call('cookies.get', cookieQuery('getAll', init));
      return list.map(cookieItem);
    },
    async set(name, value) {
      await call('cookies.set', cookieInit(name, value));
    },
    async delete(init) {
      const options = typeof init === 'object' && init !== null ? init : { name: String(init) };
      await call('cookies.delete', {
        name: String(options.name === undefined ? '' : options.name),
        domain: options.domain === undefined || options.domain === null ? null : String(options.domain),
        path: options.path === undefined || options.path === null ? '/' : String(options.path),
        partitioned: Boolean(options.partitioned),
      });
    },
  });

  const CookieChangeEvent = eventClass('CookieChangeEvent', { changed: freeze([]), deleted: freeze([]) });
  /** The store, made once, whose `change` events this context's subscription feeds. */
  let cookieStoreObject = null;
  /** Subscribe to jar changes the first time something listens on the store. */
  function watchCookies() {
    subscribe('cookies', (event) => {
      if (!cookieStoreObject) return;
      cookieStoreObject.dispatchEvent(new CookieChangeEvent('change', {
        changed: freeze(event.changed.map(cookieItem)),
        deleted: freeze(event.deleted.map(cookieItem)),
      }));
    });
  }
  const addListener = global.EventTarget.prototype.addEventListener;
  method(cookieStoreProto, 'addEventListener', function addEventListener(type, listener, options) {
    if (String(type) === 'change') watchCookies();
    return addListener.call(this, type, listener, options);
  });
  eventHandler(cookieStoreProto, 'change');

  if ((CONTEXT === 'window' || CONTEXT === 'frame' || CONTEXT === 'service') && secure()) {
    cookieStoreObject = create(cookieStoreProto);
    defineProperty(global, 'cookieStore', { get: { cookieStore() { return cookieStoreObject; } }.cookieStore, enumerable: true, configurable: true });
    defineGlobal('CookieStore', global.CookieStore || function CookieStore() { throw new TypeError('Illegal constructor'); });
  }
