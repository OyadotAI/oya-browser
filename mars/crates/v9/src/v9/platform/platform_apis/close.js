  }

  /**
   * Lazy installation: each entry point a page reaches first (a navigator member, a document
   * method, a global) is briefly an accessor that restores the original property, runs
   * `installAll`, and then reads or writes the real one. A service worker's scope installs at
   * once, since its event loop starts at bootstrap.
   */
  if (CONTEXT === 'service') {
    installAll();
    return;
  }
  host.lazy(installAll);
  /** Make `name` on the global install everything at its first read or write. Globals are the
  one entry point a native prototype cannot hold; the global object is in the heap already. */
  function trigger(name) {
    const restore = () => {
      delete global[name];
      installAll();
    };
    defineProperty(global, name, {
      configurable: true,
      enumerable: false,
      get: { [name]() { restore(); return global[name]; } }[name],
      set: { [name](value) { restore(); global[name] = value; } }[name],
    });
  }
  const GLOBAL_MEMBERS = CONTEXT === 'worker' ? ['caches'] : ['caches', 'cookieStore', 'ClipboardItem', 'PasswordCredential'];
  for (const name of GLOBAL_MEMBERS) trigger(name);
})(globalThis);
