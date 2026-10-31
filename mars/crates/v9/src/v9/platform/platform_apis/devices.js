  /**
   * Devices the coordinator reaches through macOS: Screen Wake Lock (an IOKit display-sleep
   * assertion), gamepads (the GameController framework) and Web MIDI (CoreMIDI). State that a
   * synchronous API must answer at once, such as getGamepads(), comes from the last snapshot the
   * coordinator delivered; changes arrive as events on this context's subscription.
   */
  const sentinelProto = prototypeOf('WakeLockSentinel', global.EventTarget && global.EventTarget.prototype);
  const sentinels = new Map();
  const sentinelState = new WeakMap();
  let nextSentinel = 0;
  getter(sentinelProto, 'released', (sentinel) => (sentinelState.get(sentinel) || {}).released);
  getter(sentinelProto, 'type', (sentinel) => (sentinelState.get(sentinel) || {}).type);
  eventHandler(sentinelProto, 'release');

  /** Mark a sentinel released and fire its `release` event once. */
  function releaseSentinel(sentinel) {
    const state = sentinelState.get(sentinel);
    if (!state || state.released) return;
    state.released = true;
    sentinels.delete(state.id);
    sentinel.dispatchEvent(new EventCtor('release'));
  }

  methods(sentinelProto, {
    async release() {
      const state = sentinelState.get(this);
      if (!state) throw new TypeError('Illegal invocation');
      if (!state.released) {
        await call('wakelock.release', { id: state.id });
        releaseSentinel(this);
      }
    },
  });
  methods(prototypeOf('WakeLock'), {
    async request(type) {
      const kind = type === undefined ? 'screen' : String(type);
      if (kind !== 'screen') {
        throw new TypeError(`Failed to execute 'request' on 'WakeLock': The provided value '${kind}' is not a valid enum value of type WakeLockType.`);
      }
      if (global.document && global.document.visibilityState !== 'visible') {
        throw domError('NotAllowedError', 'The requesting page is not visible');
      }
      await subscribe('wakelock', (event) => {
        for (const id of event.released || []) {
          const sentinel = sentinels.get(id);
          if (sentinel) releaseSentinel(sentinel);
        }
      });
      nextSentinel += 1;
      const id = nextSentinel;
      await call('wakelock.acquire', { id });
      const sentinel = create(sentinelProto);
      sentinelState.set(sentinel, { id, type: kind, released: false });
      sentinels.set(id, sentinel);
      return sentinel;
    },
  });

  /** Gamepad snapshots the coordinator last delivered, by index. */
  let gamepadPads = [];
  let gamepadWatching = false;
  const GAMEPAD_SLOTS = 4;
  const GamepadEvent = eventClass('GamepadEvent', { gamepad: null });

  /** A Gamepad object over one snapshot record. */
  function gamepadObject(record) {
    const pad = instance('Gamepad');
    const buttons = record.buttons.map((button) => {
      const object = instance('GamepadButton');
      for (const key of ['pressed', 'touched', 'value']) defineProperty(object, key, { value: button[key], enumerable: true });
      return object;
    });
    const fields = { id: record.id, index: record.index, connected: record.connected, mapping: record.mapping, timestamp: record.timestamp, axes: freeze(record.axes.slice()), buttons: freeze(buttons), vibrationActuator: null };
    for (const key of Object.keys(fields)) defineProperty(pad, key, { value: fields[key], enumerable: true });
    return pad;
  }

  /** Start receiving gamepad snapshots, firing connected and disconnected events at the window. */
  function watchGamepads() {
    if (gamepadWatching) return;
    gamepadWatching = true;
    subscribe('gamepad', (event) => {
      const before = new Map(gamepadPads.map((pad) => [pad.index, pad]));
      const now = global.performance.now();
      gamepadPads = event.pads.map((pad) => ({ ...pad, timestamp: now }));
      const after = new Map(gamepadPads.map((pad) => [pad.index, pad]));
      for (const [index, pad] of after) {
        if (!before.has(index)) global.dispatchEvent(new GamepadEvent('gamepadconnected', { gamepad: gamepadObject(pad) }));
      }
      for (const [index, pad] of before) {
        if (!after.has(index)) global.dispatchEvent(new GamepadEvent('gamepaddisconnected', { gamepad: gamepadObject({ ...pad, connected: false }) }));
      }
    });
  }

  if (global.navigator && (CONTEXT === 'window' || CONTEXT === 'frame')) {
    const navigatorProto = Object.getPrototypeOf(global.navigator);
    method(navigatorProto, 'getGamepads', function getGamepads() {
      watchGamepads();
      const slots = [];
      for (let index = 0; index < GAMEPAD_SLOTS; index += 1) slots.push(null);
      for (const pad of gamepadPads) slots[pad.index] = gamepadObject(pad);
      return slots;
    });
    for (const type of ['gamepadconnected', 'gamepaddisconnected']) {
      const name = 'on' + type;
      let handler = null;
      let listener = null;
      defineProperty(global, name, {
        enumerable: true,
        configurable: true,
        get: { [name]() { return handler; } }[name],
        set: { [name](value) {
          handler = typeof value === 'function' ? value : null;
          watchGamepads();
          if (handler && !listener) {
            listener = (event) => { if (handler) handler.call(global, event); };
            global.addEventListener(type, listener);
          }
        } }[name],
      });
    }
  }

  /** MIDI ports by id, and the MIDIAccess objects that list them. */
  const midiPorts = new Map();
  const midiAccesses = [];
  const portState = new WeakMap();
  const MIDIMessageEvent = eventClass('MIDIMessageEvent', { data: null });
  const MIDIConnectionEvent = eventClass('MIDIConnectionEvent', { port: null });
  const midiPortProto = prototypeOf('MIDIPort', global.EventTarget && global.EventTarget.prototype);
  for (const name of ['id', 'manufacturer', 'name', 'type', 'version', 'state', 'connection']) {
    getter(midiPortProto, name, (port) => (portState.get(port) || {})[name]);
  }
  eventHandler(midiPortProto, 'statechange');
  methods(midiPortProto, {
    async open() { return openPort(this); },
    async close() {
      const state = portState.get(this);
      if (!state) throw new TypeError('Illegal invocation');
      if (state.connection !== 'closed') {
        await call('midi.close', { id: state.id });
        setConnection(this, 'closed');
      }
      return this;
    },
  });
  const midiInputProto = prototypeOf('MIDIInput', midiPortProto);
  eventHandler(midiInputProto, 'midimessage');
  const addMidiListener = global.EventTarget.prototype.addEventListener;
  method(midiInputProto, 'addEventListener', function addEventListener(type, listener, options) {
    if (String(type) === 'midimessage') openPort(this).catch(() => {});
    return addMidiListener.call(this, type, listener, options);
  });
  const midiOutputProto = prototypeOf('MIDIOutput', midiPortProto);
  methods(midiOutputProto, {
    send(data, timestamp) {
      const state = portState.get(this);
      if (!state) throw new TypeError('Illegal invocation');
      const bytes = Array.from(data, (value) => Number(value));
      const problem = midiProblem(bytes, state.sysex);
      if (problem) throw problem;
      if (state.state === 'disconnected') throw domError('InvalidStateError', 'The port is disconnected.');
      openPort(this).catch(() => {});
      call('midi.send', { id: state.id, data: bytes, timestamp: timestamp === undefined ? 0 : Number(timestamp) }).catch(() => {});
    },
    clear() {},
  });

  /** The bytes each status byte's message takes, or 0 for a status that cannot start one. */
  function midiLength(status) {
    if (status < 0xf0) return (status & 0xf0) === 0xc0 || (status & 0xf0) === 0xd0 ? 2 : 3;
    return { 0xf1: 2, 0xf2: 3, 0xf3: 2, 0xf6: 1, 0xf8: 1, 0xfa: 1, 0xfb: 1, 0xfc: 1, 0xfe: 1, 0xff: 1 }[status] || 0;
  }

  /** Why `bytes` is not a sequence of complete MIDI messages, or null when it is. */
  function midiProblem(bytes, sysex) {
    let index = 0;
    while (index < bytes.length) {
      const status = bytes[index];
      if (!Number.isInteger(status) || status < 0x80 || status > 0xff) return new TypeError('Running status is not allowed.');
      if (status === 0xf0) {
        if (!sysex) return domError('InvalidAccessError', 'System exclusive message is not allowed.');
        const end = bytes.indexOf(0xf7, index);
        if (end < 0) return new TypeError('System exclusive message is not ended by end of system exclusive.');
        index = end + 1;
        continue;
      }
      const length = midiLength(status);
      if (!length || index + length > bytes.length) return new TypeError('Message is incomplete.');
      for (let data = index + 1; data < index + length; data += 1) {
        if (!Number.isInteger(bytes[data]) || bytes[data] > 0x7f) return new TypeError('Message is incomplete.');
      }
      index += length;
    }
    return null;
  }

  /** Change a port's connection, firing statechange at it and every MIDIAccess. */
  function setConnection(port, connection) {
    const state = portState.get(port);
    if (state.connection === connection) return;
    state.connection = connection;
    firePortChange(port);
  }

  /** statechange at the port and at each MIDIAccess, as MIDIConnectionEvents. */
  function firePortChange(port) {
    port.dispatchEvent(new MIDIConnectionEvent('statechange', { port }));
    for (const access of midiAccesses) access.dispatchEvent(new MIDIConnectionEvent('statechange', { port }));
  }

  /** Open a port implicitly or explicitly; resolves with the port. */
  async function openPort(port) {
    const state = portState.get(port);
    if (!state) throw new TypeError('Illegal invocation');
    if (state.state === 'disconnected') throw domError('InvalidAccessError', 'The port is disconnected.');
    if (state.connection !== 'open') {
      state.connection = 'pending';
      await call('midi.open', { id: state.id });
      setConnection(port, 'open');
    }
    return port;
  }

  /** The port object for one coordinator port record, made once per id. */
  function midiPort(record, sysex) {
    let port = midiPorts.get(record.id);
    if (!port) {
      port = create(record.type === 'input' ? midiInputProto : midiOutputProto);
      portState.set(port, { ...record, state: 'connected', connection: 'closed', sysex });
      midiPorts.set(record.id, port);
    }
    return port;
  }

  /** A read-only maplike of ports, shaped as MIDIInputMap or MIDIOutputMap. */
  function portMap(name, ports) {
    const map = instance(name);
    const entries = () => ports().map((port) => [portState.get(port).id, port]);
    const table = {
      get size() { return entries().length; },
      get(key) { const found = entries().find((entry) => entry[0] === key); return found ? found[1] : undefined; },
      has(key) { return entries().some((entry) => entry[0] === key); },
      keys() { return entries().map((entry) => entry[0])[Symbol.iterator](); },
      values() { return entries().map((entry) => entry[1])[Symbol.iterator](); },
      entries() { return entries()[Symbol.iterator](); },
      forEach(callback, thisArg) { for (const [key, value] of entries()) callback.call(thisArg, value, key, map); },
      [Symbol.iterator]() { return entries()[Symbol.iterator](); },
    };
    for (const key of Reflect.ownKeys(table)) defineProperty(map, key, Object.getOwnPropertyDescriptor(table, key));
    return map;
  }

  /** Apply a coordinator device change: new ports connect, vanished ports disconnect. */
  function midiDevices(event) {
    if (event.kind === 'message') {
      const port = midiPorts.get(event.port);
      const state = port && portState.get(port);
      if (state && state.connection === 'open') {
        port.dispatchEvent(new MIDIMessageEvent('midimessage', { data: new Uint8Array(event.data), timeStamp: event.timeStamp }));
      }
      return;
    }
    const live = new Set(event.ports.map((record) => record.id));
    for (const record of event.ports) {
      const known = midiPorts.has(record.id);
      const port = midiPort(record, true);
      if (!known || portState.get(port).state !== 'connected') {
        portState.get(port).state = 'connected';
        firePortChange(port);
      }
    }
    for (const [id, port] of midiPorts) {
      if (!live.has(id) && portState.get(port).state !== 'disconnected') {
        portState.get(port).state = 'disconnected';
        firePortChange(port);
      }
    }
  }

  const midiAccessProto = prototypeOf('MIDIAccess', global.EventTarget && global.EventTarget.prototype);
  eventHandler(midiAccessProto, 'statechange');
  if (global.navigator && (CONTEXT === 'window' || CONTEXT === 'frame')) {
    method(Object.getPrototypeOf(global.navigator), 'requestMIDIAccess', async function requestMIDIAccess(options) {
      const sysex = Boolean(options && options.sysex);
      const answer = await call('midi.access', { sysex });
      await subscribe('midi', midiDevices);
      for (const record of answer.ports) midiPort(record, sysex);
      const access = create(midiAccessProto);
      defineProperty(access, 'inputs', { value: portMap('MIDIInputMap', () => [...midiPorts.values()].filter((port) => portState.get(port).type === 'input')), enumerable: true });
      defineProperty(access, 'outputs', { value: portMap('MIDIOutputMap', () => [...midiPorts.values()].filter((port) => portState.get(port).type === 'output')), enumerable: true });
      defineProperty(access, 'sysexEnabled', { value: sysex, enumerable: true });
      midiAccesses.push(access);
      return access;
    });
  }
