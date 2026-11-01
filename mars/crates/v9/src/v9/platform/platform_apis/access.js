  /**
   * Storage Access API, Credential Management (password and federated credentials in the
   * coordinator's sealed store; WebAuthn rejects because Mars has no authenticator), permission
   * queries for the permissions these APIs grant, navigator.userActivation and
   * getInstalledRelatedApps, which is empty for a browser no native app is related to.
   */
  if (global.document && (CONTEXT === 'window' || CONTEXT === 'frame')) {
    const documentProto = Object.getPrototypeOf(global.document);
    methods(documentProto, {
      hasStorageAccess() { return call('storage.has', {}); },
      hasUnpartitionedCookieAccess() { return call('storage.has', {}); },
      requestStorageAccess(types) {
        if (types !== undefined && (typeof types !== 'object' || types === null)) {
          return Promise.reject(new TypeError("Failed to execute 'requestStorageAccess' on 'Document': The provided value is not of type 'StorageAccessTypes'."));
        }
        return call('storage.request', {}).then((granted) => {
          host.consume();
          return types && typeof types === 'object' && Object.keys(types).length ? storageHandle() : undefined;
        });
      },
      requestStorageAccessFor(origin) {
        if (arguments.length < 1) return Promise.reject(new TypeError("Failed to execute 'requestStorageAccessFor' on 'Document': 1 argument required, but only 0 present."));
        let url;
        try { url = new global.URL(String(origin)); } catch (error) {
          return Promise.reject(new TypeError("Failed to execute 'requestStorageAccessFor' on 'Document': Invalid origin"));
        }
        return call('storage.requestFor', { origin: url.origin }).then(() => undefined);
      },
    });
  }

  /** The StorageAccessHandle a typed request resolves with: this document's own storage. */
  function storageHandle() {
    return freeze({ localStorage: global.localStorage, sessionStorage: global.sessionStorage });
  }

  const credentialProto = prototypeOf('Credential');
  const credentialData = new WeakMap();
  /** A credential's state, or a TypeError for another receiver. */
  function credentialState(credential) {
    const state = credentialData.get(credential);
    if (!state) throw new TypeError('Illegal invocation');
    return state;
  }
  getter(credentialProto, 'id', (credential) => credentialState(credential).id);
  getter(credentialProto, 'type', (credential) => credentialState(credential).type);

  /** A Credential subclass constructor named `name` of `type` whose `fields` read from its state. */
  function credentialClass(name, type, fields, build) {
    const ctor = {
      [name]: function (init) {
        if (!new.target) throw new TypeError(`Failed to construct '${name}': Please use the 'new' operator.`);
        credentialData.set(this, { type, ...build(init) });
      },
    }[name];
    ctor.prototype = prototypeOf(name, credentialProto);
    defineProperty(ctor.prototype, 'constructor', { value: ctor, writable: true, configurable: true });
    for (const field of fields) getter(ctor.prototype, field, (credential) => credentialState(credential)[field]);
    defineGlobal(name, ctor);
    return ctor;
  }

  /** PasswordCredentialData from a dictionary or a form's autocomplete-labelled fields. */
  function passwordData(init) {
    let data = init;
    if (global.HTMLFormElement && init instanceof global.HTMLFormElement) data = formCredential(init);
    if (typeof data !== 'object' || data === null) throw new TypeError("Failed to construct 'PasswordCredential': The provided value is not of type 'PasswordCredentialData'.");
    const id = data.id === undefined ? '' : String(data.id);
    const password = data.password === undefined ? '' : String(data.password);
    if (!id) throw new TypeError("Failed to construct 'PasswordCredential': 'id' must not be empty.");
    if (!password) throw new TypeError("Failed to construct 'PasswordCredential': 'password' must not be empty.");
    return { id, password, name: data.name === undefined ? '' : String(data.name), iconURL: data.iconURL === undefined ? '' : String(data.iconURL) };
  }

  /** The id, password, name and icon a form's fields carry, by their autocomplete tokens. */
  function formCredential(form) {
    const data = {};
    for (const field of form.elements) {
      const tokens = String(field.getAttribute('autocomplete') || '').split(/\s+/);
      if (tokens.includes('username')) data.id = field.value;
      if (tokens.includes('current-password') || tokens.includes('new-password')) data.password = field.value;
      if (tokens.includes('name') || tokens.includes('nickname')) data.name = field.value;
      if (tokens.includes('photo')) data.iconURL = field.value;
    }
    return data;
  }

  /** FederatedCredentialData from a dictionary. */
  function federatedData(init) {
    if (typeof init !== 'object' || init === null) throw new TypeError("Failed to construct 'FederatedCredential': The provided value is not of type 'FederatedCredentialInit'.");
    const id = init.id === undefined ? '' : String(init.id);
    const provider = init.provider === undefined ? '' : String(init.provider);
    if (!id) throw new TypeError("Failed to construct 'FederatedCredential': 'id' must not be empty.");
    if (!provider) throw new TypeError("Failed to construct 'FederatedCredential': 'provider' must not be empty.");
    return { id, provider, protocol: init.protocol === undefined ? null : String(init.protocol), name: init.name === undefined ? '' : String(init.name), iconURL: init.iconURL === undefined ? '' : String(init.iconURL) };
  }

  const PasswordCredential = credentialClass('PasswordCredential', 'password', ['password', 'name', 'iconURL'], passwordData);
  const FederatedCredential = credentialClass('FederatedCredential', 'federated', ['provider', 'protocol', 'name', 'iconURL'], federatedData);

  /** A credential object from the coordinator's stored record. */
  function credentialFrom(record) {
    if (!record) return null;
    return record.type === 'federated' ? new FederatedCredential(record) : new PasswordCredential(record);
  }

  /** The DOMException a WebAuthn ceremony ends with when no authenticator can answer. */
  function noAuthenticator() {
    return domError('NotAllowedError', 'The operation either timed out or was not allowed. See: https://www.w3.org/TR/webauthn-2/#sctn-privacy-considerations-client.');
  }

  const credentialsProto = prototypeOf('CredentialsContainer');
  methods(credentialsProto, {
    async get(options) {
      const request = options === undefined ? {} : options;
      const aborted = abortReason(request.signal);
      if (aborted) throw aborted;
      if (request.publicKey) throw noAuthenticator();
      if (request.identity || request.otp) throw domError('NotSupportedError', 'This credential type is not supported.');
      const providers = request.federated && isArray(request.federated.providers) ? request.federated.providers.map(String) : [];
      if (!request.password && providers.length === 0) return null;
      const mediation = request.mediation === undefined ? 'optional' : String(request.mediation);
      return credentialFrom(await call('credentials.get', { password: Boolean(request.password), providers, mediation }));
    },
    async create(options) {
      const request = options === undefined ? {} : options;
      const aborted = abortReason(request.signal);
      if (aborted) throw aborted;
      if (request.publicKey) throw noAuthenticator();
      if (request.password) return new PasswordCredential(request.password);
      if (request.federated) return new FederatedCredential(request.federated);
      throw domError('NotSupportedError', 'Only exactly one of \'password\', \'federated\' or \'publicKey\' may be given.');
    },
    async store(credential) {
      const state = credentialState(credential);
      await call('credentials.store', { ...state });
      return credential;
    },
    async preventSilentAccess() {
      await call('credentials.preventSilentAccess', {});
    },
  });
  if (global.PublicKeyCredential) {
    method(global.PublicKeyCredential, 'isUserVerifyingPlatformAuthenticatorAvailable', function isUserVerifyingPlatformAuthenticatorAvailable() { return Promise.resolve(false); });
    method(global.PublicKeyCredential, 'isConditionalMediationAvailable', function isConditionalMediationAvailable() { return Promise.resolve(false); });
    method(global.PublicKeyCredential, 'getClientCapabilities', function getClientCapabilities() {
      return Promise.resolve({ conditionalCreate: false, conditionalGet: false, hybridTransport: false, passkeyPlatformAuthenticator: false, userVerifyingPlatformAuthenticator: false, relatedOrigins: false, signalAllAcceptedCredentials: false, signalCurrentUserDetails: false, signalUnknownCredential: false });
    });
  }
  method(credentialProto.constructor || global.Credential || {}, 'isConditionalMediationAvailable', function isConditionalMediationAvailable() { return Promise.resolve(false); });

  /** Permissions whose state this bootstrap's APIs decide; the rest keep their native answer. */
  const OWN_PERMISSIONS = ['midi', 'storage-access', 'clipboard-read', 'clipboard-write', 'screen-wake-lock', 'top-level-storage-access'];
  const permissionsProto = prototypeOf('Permissions');
  const nativeQuery = permissionsProto.query;
  method(permissionsProto, 'query', function query(descriptor) {
    if (typeof descriptor !== 'object' || descriptor === null) return nativeQuery.call(this, descriptor);
    const name = String(descriptor.name);
    if (!OWN_PERMISSIONS.includes(name)) return nativeQuery.call(this, descriptor);
    return call('permissions.query', { name, sysex: Boolean(descriptor.sysex) }).then((state) => {
      const status = instance('PermissionStatus', global.EventTarget && global.EventTarget.prototype);
      defineProperty(status, 'name', { value: name, enumerable: true });
      defineProperty(status, 'state', { value: state, enumerable: true });
      defineProperty(status, 'onchange', { value: null, writable: true, enumerable: true });
      return status;
    });
  });


  if (global.navigator && CONTEXT !== 'service') {
    method(Object.getPrototypeOf(global.navigator), 'getInstalledRelatedApps', function getInstalledRelatedApps() {
      if (CONTEXT !== 'window') {
        return Promise.reject(domError('InvalidStateError', 'getInstalledRelatedApps() is only supported in top-level browsing contexts.'));
      }
      return Promise.resolve([]);
    });
  }
