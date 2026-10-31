  /**
   * Encrypted Media Extensions with the Clear Key key system (`org.w3.clearkey`), following the
   * EME algorithms: requestMediaKeySystemAccess runs "Get Supported Configuration", sessions
   * build license requests from cenc, keyids and webm initialization data and accept JWK set
   * licenses. Usable keys go to the coordinator's Clear Key store (`eme.keys`), which is the
   * media pipeline's decryption interface. Every other key system (Widevine among them) is not
   * available and rejects with NotSupportedError as the algorithm specifies.
   */
  const CLEAR_KEY = 'org.w3.clearkey';
  const INIT_DATA_TYPES = ['cenc', 'keyids', 'webm'];
  /** The Common PSSH system ID, under which cenc init data lists its key IDs. */
  const COMMON_SYSTEM = '1077efecc0b24d02ace33c1e52e2fb4b';
  /** Containers and the codecs Mars's media stack names for them. */
  const EME_CODECS = {
    'video/mp4': ['avc1', 'avc3', 'hev1', 'hvc1', 'vp09', 'av01', 'mp4a', 'opus', 'flac'],
    'audio/mp4': ['mp4a', 'opus', 'flac'],
    'video/webm': ['vp8', 'vp9', 'vp09', 'av01', 'opus', 'vorbis'],
    'audio/webm': ['opus', 'vorbis'],
  };
  const KEY_ID_BYTES = 16;
  const PSSH_HEADER = 8;
  const MAX_WEBM_KEY_ID = 512;

  /** Whether one MediaKeySystemMediaCapability is supported by Clear Key. */
  function capabilitySupported(capability) {
    if (String(capability.robustness || '') !== '') return false;
    const scheme = capability.encryptionScheme;
    if (scheme !== undefined && scheme !== null && scheme !== 'cenc' && scheme !== 'cbcs' && scheme !== 'cbcs-1-9') return false;
    const match = /^\s*([a-z]+\/[a-z0-9.+-]+)\s*(?:;\s*codecs\s*=\s*"?([^"]*)"?)?\s*$/i.exec(String(capability.contentType || ''));
    if (!match) return false;
    const codecs = EME_CODECS[match[1].toLowerCase()];
    if (!codecs) return false;
    const listed = (match[2] || '').split(',').map((codec) => codec.trim().split('.')[0].toLowerCase()).filter(Boolean);
    return listed.every((codec) => codecs.includes(codec));
  }

  /** EME's "Get Supported Configuration" for Clear Key: the accumulated configuration or null. */
  function supportedConfiguration(candidate) {
    const config = candidate || {};
    const initDataTypes = config.initDataTypes === undefined ? [] : Array.from(config.initDataTypes, String);
    const supportedTypes = initDataTypes.filter((type) => INIT_DATA_TYPES.includes(type));
    if (initDataTypes.length && !supportedTypes.length) return null;
    if (config.distinctiveIdentifier === 'required' || config.persistentState === 'required') return null;
    const sessionTypes = config.sessionTypes === undefined ? ['temporary'] : Array.from(config.sessionTypes, String);
    if (sessionTypes.some((type) => type !== 'temporary')) return null;
    const pick = (list) => (list === undefined ? [] : Array.from(list).filter(capabilitySupported));
    const video = pick(config.videoCapabilities);
    const audio = pick(config.audioCapabilities);
    if (config.videoCapabilities && config.videoCapabilities.length && !video.length) return null;
    if (config.audioCapabilities && config.audioCapabilities.length && !audio.length) return null;
    if (!video.length && !audio.length) return null;
    const capability = (item) => ({ contentType: String(item.contentType), encryptionScheme: item.encryptionScheme === undefined ? null : item.encryptionScheme, robustness: '' });
    return {
      label: config.label === undefined ? '' : String(config.label),
      initDataTypes: supportedTypes,
      audioCapabilities: audio.map(capability),
      videoCapabilities: video.map(capability),
      distinctiveIdentifier: 'not-allowed',
      persistentState: 'not-allowed',
      sessionTypes,
    };
  }

  const accessProto = prototypeOf('MediaKeySystemAccess');
  const accessState = new WeakMap();
  getter(accessProto, 'keySystem', (access) => (accessState.get(access) || {}).keySystem);
  const mediaKeysProto = prototypeOf('MediaKeys');
  const sessionProto = prototypeOf('MediaKeySession', global.EventTarget && global.EventTarget.prototype);
  const sessionState = new WeakMap();
  const MediaKeyMessageEvent = eventClass('MediaKeyMessageEvent', { messageType: 'license-request', message: null });
  methods(accessProto, {
    getConfiguration() {
      const config = accessState.get(this).configuration;
      return fromJson(toJson(config));
    },
    async createMediaKeys() {
      if (!accessState.get(this)) throw new TypeError('Illegal invocation');
      return create(mediaKeysProto);
    },
  });
  let nextSession = 0;
  methods(mediaKeysProto, {
    createSession(sessionType) {
      const type = sessionType === undefined ? 'temporary' : String(sessionType);
      if (type !== 'temporary' && type !== 'persistent-license') {
        throw new TypeError(`Failed to execute 'createSession' on 'MediaKeys': The provided value '${type}' is not a valid enum value of type MediaKeySessionType.`);
      }
      if (type !== 'temporary') throw domError('NotSupportedError', 'Unsupported session type.');
      const session = create(sessionProto);
      let closedResolve;
      const closed = new Promise((resolve) => { closedResolve = resolve; });
      sessionState.set(session, { id: '', closed: false, requested: false, keys: new Map(), closedPromise: closed, closedResolve, statuses: keyStatusMap(() => sessionState.get(session).keys) });
      return session;
    },
    async setServerCertificate(certificate) {
      bufferBytes(certificate, "Failed to execute 'setServerCertificate' on 'MediaKeys': parameter 1");
      return false;
    },
    async getStatusForPolicy() { return 'usable'; },
  });

  /** A MediaKeyStatusMap over a session's key table (key id hex to status). */
  function keyStatusMap(table) {
    const map = instance('MediaKeyStatusMap');
    const hex = (keyId) => Array.from(bufferBytes(keyId, 'keyId'), (byte) => byte.toString(16).padStart(2, '0')).join('');
    const entries = () => [...table()].map(([id, entry]) => [fromHex(id).buffer, entry.status]);
    const members = {
      get size() { return table().size; },
      get(keyId) { const entry = table().get(hex(keyId)); return entry ? entry.status : undefined; },
      has(keyId) { return table().has(hex(keyId)); },
      keys() { return entries().map((entry) => entry[0])[Symbol.iterator](); },
      values() { return entries().map((entry) => entry[1])[Symbol.iterator](); },
      entries() { return entries()[Symbol.iterator](); },
      forEach(callback, thisArg) { for (const [key, value] of entries()) callback.call(thisArg, value, key, map); },
      [Symbol.iterator]() { return entries()[Symbol.iterator](); },
    };
    for (const key of Reflect.ownKeys(members)) defineProperty(map, key, Object.getOwnPropertyDescriptor(members, key));
    return map;
  }

  /** Lowercase hex of bytes. */
  function toHex(bytes) {
    return Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('');
  }

  /** Bytes of lowercase hex. */
  function fromHex(text) {
    const bytes = new Uint8Array(text.length / 2);
    for (let i = 0; i < bytes.length; i += 1) bytes[i] = parseInt(text.substr(i * 2, 2), 16);
    return bytes;
  }

  /** Key IDs (hex) in cenc initialization data: the Common-system version 1 PSSH boxes. */
  function cencKeyIds(bytes) {
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    const ids = [];
    let offset = 0;
    while (offset + PSSH_HEADER <= bytes.length) {
      const size = view.getUint32(offset);
      const type = String.fromCharCode(...bytes.subarray(offset + 4, offset + 8));
      if (size < PSSH_HEADER || offset + size > bytes.length || type !== 'pssh') return null;
      const version = bytes[offset + 8];
      const system = toHex(bytes.subarray(offset + 12, offset + 28));
      if (version > 0 && system === COMMON_SYSTEM) {
        const count = view.getUint32(offset + 28);
        for (let i = 0; i < count; i += 1) {
          const start = offset + 32 + i * KEY_ID_BYTES;
          if (start + KEY_ID_BYTES > offset + size) return null;
          ids.push(toHex(bytes.subarray(start, start + KEY_ID_BYTES)));
        }
      }
      offset += size;
    }
    return offset === bytes.length ? ids : null;
  }

  /** Key IDs (hex) the initialization data names, or null when it is malformed. */
  function initDataKeyIds(type, bytes) {
    if (type === 'cenc') return cencKeyIds(bytes);
    if (type === 'webm') return bytes.length && bytes.length <= MAX_WEBM_KEY_ID ? [toHex(bytes)] : null;
    try {
      const parsed = JSON.parse(new global.TextDecoder().decode(bytes));
      if (!isArray(parsed.kids)) return null;
      const ids = parsed.kids.map((kid) => fromBase64Url(kid));
      return ids.every((id) => id && id.length) ? ids.map(toHex) : null;
    } catch (error) { return null; }
  }

  /** A session's state, or the InvalidStateError a closed or foreign session gets. */
  function liveSession(session, needsRequest) {
    const state = sessionState.get(session);
    if (!state) throw new TypeError('Illegal invocation');
    if (state.closed) throw domError('InvalidStateError', 'The session is already closed.');
    if (needsRequest && !state.requested) throw domError('InvalidStateError', 'The session is not yet initialized.');
    return state;
  }

  /** Set every key's status and fire keystatuseschange. */
  function setStatuses(session, state, status) {
    for (const entry of state.keys.values()) entry.status = status;
    session.dispatchEvent(new EventCtor('keystatuseschange'));
  }

  getter(sessionProto, 'sessionId', (session) => (sessionState.get(session) || {}).id);
  getter(sessionProto, 'expiration', () => NaN);
  getter(sessionProto, 'closed', (session) => (sessionState.get(session) || {}).closedPromise);
  getter(sessionProto, 'keyStatuses', (session) => (sessionState.get(session) || {}).statuses);
  eventHandler(sessionProto, 'message');
  eventHandler(sessionProto, 'keystatuseschange');
  methods(sessionProto, {
    async generateRequest(initDataType, initData) {
      const state = liveSession(this, false);
      if (state.requested) throw domError('InvalidStateError', 'A request has already been generated.');
      const type = String(initDataType);
      if (!type) throw new TypeError("Failed to execute 'generateRequest' on 'MediaKeySession': The initDataType parameter is empty.");
      const bytes = bufferBytes(initData, "Failed to execute 'generateRequest' on 'MediaKeySession': parameter 2");
      if (!bytes.length) throw new TypeError("Failed to execute 'generateRequest' on 'MediaKeySession': The initData parameter is empty.");
      if (!INIT_DATA_TYPES.includes(type)) throw domError('NotSupportedError', 'The initialization data type is not supported by the key system.');
      const ids = initDataKeyIds(type, bytes);
      if (!ids || !ids.length) throw new TypeError('Initialization data did not contain any key ID.');
      nextSession += 1;
      state.requested = true;
      state.id = String(nextSession);
      state.requestedIds = ids;
      const message = new global.TextEncoder().encode(toJson({ kids: ids.map((id) => toBase64Url(fromHex(id))), type: 'temporary' }));
      // The message event is its own task, queued as the promise resolves.
      global.setTimeout(() => this.dispatchEvent(new MediaKeyMessageEvent('message', { messageType: 'license-request', message: message.buffer })), 0);
    },
    async load() {
      liveSession(this, false);
      throw new TypeError('The session type does not support load().');
    },
    async update(response) {
      const state = liveSession(this, true);
      const bytes = bufferBytes(response, "Failed to execute 'update' on 'MediaKeySession': parameter 1");
      if (!bytes.length) throw new TypeError("Failed to execute 'update' on 'MediaKeySession': The response parameter is empty.");
      let license;
      try { license = JSON.parse(new global.TextDecoder().decode(bytes)); } catch (error) { license = null; }
      const keys = license && isArray(license.keys) ? license.keys : null;
      if (!keys || !keys.length) throw new TypeError('The license is not a valid JSON Web Key Set.');
      const accepted = [];
      for (const key of keys) {
        const id = fromBase64Url(key.kid);
        const value = fromBase64Url(key.k);
        if (key.kty !== 'oct' || !id || !id.length || !value || value.length !== KEY_ID_BYTES) {
          throw new TypeError('The license is not a valid JSON Web Key Set.');
        }
        accepted.push({ kid: toHex(id), key: toHex(value) });
      }
      await call('eme.keys', { session: state.id, keys: accepted });
      for (const key of accepted) state.keys.set(key.kid, { status: 'usable' });
      this.dispatchEvent(new EventCtor('keystatuseschange'));
    },
    async close() {
      const state = sessionState.get(this);
      if (!state) throw new TypeError('Illegal invocation');
      if (state.closed) return;
      state.closed = true;
      if (state.requested) await call('eme.close', { session: state.id });
      state.keys.clear();
      this.dispatchEvent(new EventCtor('keystatuseschange'));
      state.closedResolve('closed-by-application');
    },
    async remove() {
      const state = liveSession(this, true);
      await call('eme.close', { session: state.id });
      setStatuses(this, state, 'released');
    },
  });

  if (global.navigator && (CONTEXT === 'window' || CONTEXT === 'frame')) {
    method(Object.getPrototypeOf(global.navigator), 'requestMediaKeySystemAccess', function requestMediaKeySystemAccess(keySystem, supportedConfigurations) {
      if (arguments.length < 2) return Promise.reject(new TypeError("Failed to execute 'requestMediaKeySystemAccess' on 'Navigator': 2 arguments required."));
      const system = String(keySystem);
      if (!system) return Promise.reject(new TypeError("Failed to execute 'requestMediaKeySystemAccess' on 'Navigator': The keySystem parameter is empty."));
      const configs = Array.from(supportedConfigurations || []);
      if (!configs.length) return Promise.reject(new TypeError("Failed to execute 'requestMediaKeySystemAccess' on 'Navigator': The supportedConfigurations parameter is empty."));
      return Promise.resolve().then(() => {
        const configuration = system === CLEAR_KEY ? configs.map(supportedConfiguration).find(Boolean) : null;
        if (!configuration) throw domError('NotSupportedError', 'Unsupported keySystem or supportedConfigurations.');
        const access = create(accessProto);
        accessState.set(access, { keySystem: system, configuration });
        return access;
      });
    });
    const mediaProto = global.HTMLMediaElement && global.HTMLMediaElement.prototype;
    const attached = new WeakMap();
    const owners = new WeakMap();
    if (mediaProto) {
      method(mediaProto, 'setMediaKeys', async function setMediaKeys(mediaKeys) {
        if (mediaKeys !== null && Object.getPrototypeOf(mediaKeys) !== mediaKeysProto) {
          throw new TypeError("Failed to execute 'setMediaKeys' on 'HTMLMediaElement': parameter 1 is not of type 'MediaKeys'.");
        }
        const owner = mediaKeys && owners.get(mediaKeys);
        if (owner && owner !== this) throw domError('QuotaExceededError', 'The MediaKeys object is already in use by another media element.');
        const previous = attached.get(this);
        if (previous) owners.delete(previous);
        if (mediaKeys) { attached.set(this, mediaKeys); owners.set(mediaKeys, this); } else attached.delete(this);
      });
      getter(mediaProto, 'mediaKeys', (element) => attached.get(element) || null);
    }
  }
