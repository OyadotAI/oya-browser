  /**
   * Async Clipboard API, ClipboardItem, DataTransfer, ClipboardEvent and execCommand's copy, cut
   * and paste. The system pasteboard sits in the coordinator; every read and write needs a
   * focused document and transient user activation, as Chrome requires, or it rejects with
   * NotAllowedError there.
   */
  const CLIPBOARD_TYPES = ['text/plain', 'text/html', 'image/png'];

  /** ClipboardItem: a map from MIME type to a Blob, string, or a promise of either. */
  const itemData = new WeakMap();
  function ClipboardItem(items, options) {
    if (!new.target) throw new TypeError("Failed to construct 'ClipboardItem': Please use the 'new' operator.");
    if (typeof items !== 'object' || items === null) {
      throw new TypeError("Failed to construct 'ClipboardItem': The provided value is not of type 'record<DOMString, (DOMString or Blob or Promise<(DOMString or Blob)>)>'.");
    }
    const types = Object.keys(items);
    if (types.length === 0) throw new TypeError("Failed to construct 'ClipboardItem': Empty dictionary argument");
    const presentation = options && options.presentationStyle ? String(options.presentationStyle) : 'unspecified';
    itemData.set(this, { items: { ...items }, types: freeze(types.slice()), presentation });
  }
  ClipboardItem.prototype = prototypeOf('ClipboardItem');
  defineProperty(ClipboardItem.prototype, 'constructor', { value: ClipboardItem, writable: true, configurable: true });
  method(ClipboardItem, 'supports', function supports(type) { return CLIPBOARD_TYPES.includes(String(type)) || String(type).startsWith('web '); });
  getter(ClipboardItem.prototype, 'types', (item) => itemState(item).types);
  getter(ClipboardItem.prototype, 'presentationStyle', (item) => itemState(item).presentation);
  methods(ClipboardItem.prototype, {
    async getType(type) {
      const state = itemState(this);
      const name = String(type);
      if (!Object.prototype.hasOwnProperty.call(state.items, name)) {
        throw domError('NotFoundError', "Failed to execute 'getType' on 'ClipboardItem': The type was not found");
      }
      return asBlob(await state.items[name], name);
    },
  });
  defineGlobal('ClipboardItem', ClipboardItem);

  /** The internal state of a ClipboardItem, or a TypeError for another object. */
  function itemState(item) {
    const state = itemData.get(item);
    if (!state) throw new TypeError('Illegal invocation');
    return state;
  }

  /** A string or Blob as a Blob of `type`. */
  function asBlob(value, type) {
    if (value instanceof global.Blob) return value;
    return new global.Blob([String(value)], { type });
  }

  /** Read one representation for the pasteboard: text as text, images as bytes. */
  async function representation(type, value) {
    const blob = asBlob(await value, type);
    if (type.startsWith('text/')) return { type, text: await blob.text() };
    if (type === 'image/png') return { type, base64: toBase64(await blob.arrayBuffer()) };
    throw domError('NotAllowedError', `Type ${type} not supported on write.`);
  }

  /** The focus-and-activation gate both directions share; the coordinator checks it again. */
  function clipboardGate(what) {
    if (global.document && !global.document.hasFocus()) {
      return Promise.reject(domError('NotAllowedError', 'Document is not focused.'));
    }
    return null;
  }

  const clipboardProto = prototypeOf('Clipboard', global.EventTarget && global.EventTarget.prototype);
  methods(clipboardProto, {
    writeText(data) {
      if (arguments.length < 1) return Promise.reject(new TypeError("Failed to execute 'writeText' on 'Clipboard': 1 argument required, but only 0 present."));
      return clipboardGate('write') || call('clipboard.write', { items: [{ type: 'text/plain', text: String(data) }] }).then(() => undefined);
    },
    async write(data) {
      const gated = clipboardGate('write');
      if (gated) return gated;
      if (!isArray(data) && !(data && typeof data[Symbol.iterator] === 'function')) {
        throw new TypeError("Failed to execute 'write' on 'Clipboard': The provided value cannot be converted to a sequence.");
      }
      const items = [];
      for (const item of data) {
        const state = itemState(item);
        for (const type of state.types) items.push(await representation(type, state.items[type]));
      }
      await call('clipboard.write', { items });
    },
    readText() {
      return clipboardGate('read') || call('clipboard.read', { types: ['text/plain'] }).then((items) => {
        const text = (items || []).find((item) => item.type === 'text/plain');
        return text ? text.text : '';
      });
    },
    read() {
      return clipboardGate('read') || call('clipboard.read', { types: CLIPBOARD_TYPES }).then((items) => {
        if (!items || items.length === 0) return [];
        const record = {};
        for (const item of items) {
          record[item.type] = item.text !== undefined
            ? new global.Blob([item.text], { type: item.type })
            : new global.Blob([fromBase64(item.base64)], { type: item.type });
        }
        return [new ClipboardItem(record)];
      });
    },
  });
  eventHandler(clipboardProto, 'clipboardchange');

  /** DataTransfer as clipboard events carry it: ordered string data by type, no files. */
  const transfers = new WeakMap();
  function DataTransfer() {
    if (!new.target) throw new TypeError("Failed to construct 'DataTransfer': Please use the 'new' operator.");
    transfers.set(this, { data: new Map(), dropEffect: 'none', effectAllowed: 'uninitialized' });
  }
  DataTransfer.prototype = prototypeOf('DataTransfer');
  defineProperty(DataTransfer.prototype, 'constructor', { value: DataTransfer, writable: true, configurable: true });
  /** The state of a DataTransfer this bootstrap made. */
  function transfer(object) {
    const state = transfers.get(object);
    if (!state) throw new TypeError('Illegal invocation');
    return state;
  }
  /** HTML's type normalization: lowercase, with "text" and "url" aliases. */
  function transferType(type) {
    const name = String(type).toLowerCase();
    if (name === 'text') return 'text/plain';
    if (name === 'url') return 'text/uri-list';
    return name;
  }
  methods(DataTransfer.prototype, {
    setData(type, data) { transfer(this).data.set(transferType(type), String(data)); },
    getData(type) { const value = transfer(this).data.get(transferType(type)); return value === undefined ? '' : value; },
    clearData(type) {
      const state = transfer(this);
      if (type === undefined) state.data.clear(); else state.data.delete(transferType(type));
    },
    setDragImage() {},
  });
  getter(DataTransfer.prototype, 'types', (object) => freeze([...transfer(object).data.keys()]));
  getter(DataTransfer.prototype, 'files', () => freeze([]));
  getter(DataTransfer.prototype, 'items', (object) => freeze([...transfer(object).data.keys()].map((type) => ({ kind: 'string', type }))));
  for (const name of ['dropEffect', 'effectAllowed']) {
    defineProperty(DataTransfer.prototype, name, {
      enumerable: true,
      configurable: true,
      get() { return transfer(this)[name]; },
      set(value) { transfer(this)[name] = String(value); },
    });
  }
  defineGlobal('DataTransfer', DataTransfer);
  const ClipboardEvent = eventClass('ClipboardEvent', { clipboardData: null });

  /** Text the person selected: a text control's selection, else the document selection. */
  function selectedText() {
    const document = global.document;
    const active = document && document.activeElement;
    if (active && typeof active.selectionStart === 'number' && typeof active.value === 'string') {
      return active.value.slice(active.selectionStart, active.selectionEnd);
    }
    const selection = global.getSelection && global.getSelection();
    return selection ? String(selection) : '';
  }

  /** execCommand('copy'|'cut'): fire the event, then write its data or the selection. */
  function clipboardCommand(kind) {
    if (!isActive()) return false;
    const document = global.document;
    const data = new DataTransfer();
    const event = new ClipboardEvent(kind, { bubbles: true, cancelable: true, composed: true, clipboardData: data });
    const target = document.activeElement || document.body || document.documentElement;
    const proceed = target.dispatchEvent(event);
    const items = proceed
      ? [{ type: 'text/plain', text: selectedText() }]
      : data.types.map((type) => ({ type, text: data.getData(type) })).filter((item) => CLIPBOARD_TYPES.includes(item.type));
    if (proceed && kind === 'cut') removeSelection();
    if (items.length) call('clipboard.write', { items }).catch(() => {});
    return true;
  }

  /** Cut's default action in an editable text control: delete the selected text. */
  function removeSelection() {
    const active = global.document.activeElement;
    if (!active || typeof active.selectionStart !== 'number' || typeof active.value !== 'string') return;
    const start = active.selectionStart;
    active.value = active.value.slice(0, start) + active.value.slice(active.selectionEnd);
    active.selectionStart = active.selectionEnd = start;
  }

  if (global.document && CONTEXT !== 'worker' && CONTEXT !== 'service') {
    const documentProto = Object.getPrototypeOf(global.document);
    const previousExec = documentProto.execCommand;
    const previousSupported = documentProto.queryCommandSupported;
    const previousEnabled = documentProto.queryCommandEnabled;
    const commands = { copy: true, cut: true, paste: true };
    /** Defer a command this file does not own to whatever the document had before. */
    const defer = (previous, self, args) => (typeof previous === 'function' ? Boolean(previous.apply(self, args)) : false);
    methods(documentProto, {
      execCommand(command, showUI, value) {
        const name = String(command).toLowerCase();
        if (name === 'copy' || name === 'cut') return clipboardCommand(name);
        // Chrome refuses paste to ordinary web content: it needs an extension's clipboardRead permission.
        if (name === 'paste') return false;
        return defer(previousExec, this, [command, showUI, value]);
      },
      queryCommandSupported(command) {
        return commands[String(command).toLowerCase()] === true || defer(previousSupported, this, [command]);
      },
      queryCommandEnabled(command) {
        const name = String(command).toLowerCase();
        if (commands[name]) return name !== 'paste' && isActive();
        return defer(previousEnabled, this, [command]);
      },
    });
  }
