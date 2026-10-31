
  /**
   * Interface shapes. Each IndexedDB interface is a function that throws "Illegal constructor",
   * with a prototype whose operations and attributes are enumerable, as WebIDL defines them and
   * Chrome exposes them, and a Symbol.toStringTag naming the interface.
   */

  /** Create the interface object and prototype for `name`, inheriting from `parent` (an interface object) when given. */
  function makeInterface(name, parent, constructor) {
    const ctor = constructor || { [name]: function () { throw new NativeTypeError('Illegal constructor'); } }[name];
    const proto = ObjectCreate(parent ? parent.prototype : Object.prototype);
    defineProperty(ctor, 'prototype', { value: proto, writable: false, enumerable: false, configurable: false });
    defineProperty(proto, 'constructor', { value: ctor, writable: true, enumerable: false, configurable: true });
    defineProperty(proto, Symbol.toStringTag, { value: name, writable: false, enumerable: false, configurable: true });
    if (parent) {
      setPrototypeOf(ctor, parent);
    }
    return ctor;
  }

  /** Copy each own member of `source` (methods and accessors) onto `target` as enumerable, configurable WebIDL members. */
  function members(target, source) {
    for (const name of Reflect.ownKeys(source)) {
      const descriptor = getOwnPropertyDescriptor(source, name);
      descriptor.enumerable = true;
      descriptor.configurable = true;
      defineProperty(target, name, descriptor);
    }
  }

  /** Install `on<type>` event handler attributes; a handler joins the listener list the first time one is set, as HTML specifies. */
  function eventHandlers(proto, types) {
    for (const type of types) {
      const name = 'on' + type;
      const descriptor = {
        get() { return handlerTable(this)[name] ?? null; },
        set(value) { setHandler(this, type, isObject(value) ? value : null); },
        enumerable: true,
        configurable: true,
      };
      defineProperty(descriptor.get, 'name', { value: 'get ' + name });
      defineProperty(descriptor.set, 'name', { value: 'set ' + name });
      defineProperty(proto, name, descriptor);
    }
  }

  /** The handler table of an event target this file built; any other receiver is an illegal invocation. */
  function handlerTable(target) {
    const state = typeof target === 'object' && target !== null ? apply(weakGet, internals, [target]) : undefined;
    if (!state || !state.handlers) {
      throw new NativeTypeError('Illegal invocation');
    }
    return state.handlers;
  }

  /** Store a handler, registering the one listener that invokes whatever handler is current. */
  function setHandler(target, type, value) {
    const table = handlerTable(target);
    const name = 'on' + type;
    const registered = own(table, name);
    table[name] = value;
    if (!registered && value !== null) {
      call(addListener, target, type, function (event) {
        const handler = table[name];
        return typeof handler === 'function' ? call(handler, this, event) : undefined;
      });
    } else if (!registered) {
      delete table[name];
    }
  }

  /** A new instance of an EventTarget-derived interface carrying `state`. */
  function eventTargetInstance(ctor, state) {
    state.handlers = ObjectCreate(null);
    return bind(construct(NativeEventTarget, [], ctor), state);
  }

  /** A new instance of a plain interface carrying `state`. */
  function plainInstance(ctor, state) {
    return bind(ObjectCreate(ctor.prototype), state);
  }

  /** DOMStringList: an immutable, indexable list of names, sorted as IndexedDB reports store and index names. */
  const DOMStringList = makeInterface('DOMStringList', null);
  members(DOMStringList.prototype, {
    get length() { return stateOf(this, DOMStringList).names.length; },
    item(index) {
      const names = stateOf(this, DOMStringList).names;
      const position = toUnsignedLong(index);
      return position < names.length ? names[position] : null;
    },
    contains(string) {
      const names = stateOf(this, DOMStringList).names;
      return names.includes(String(string));
    },
  });
  defineProperty(DOMStringList.prototype, Symbol.iterator, {
    value: Array.prototype.values, writable: true, enumerable: false, configurable: true,
  });

  /** A DOMStringList over `names`, which the caller has already sorted; indexes are own read-only properties. */
  function stringList(names) {
    const list = plainInstance(DOMStringList, { brand: DOMStringList, names });
    for (let index = 0; index < names.length; index++) {
      defineProperty(list, index, { value: names[index], writable: false, enumerable: true, configurable: true });
    }
    return list;
  }

  /** WebIDL's unsigned long conversion, as `item()` applies it. */
  function toUnsignedLong(value) {
    const number = Number(value);
    if (!Number.isFinite(number)) {
      return 0;
    }
    const UINT32 = 4294967296;
    const integer = Math.trunc(number) % UINT32;
    return integer < 0 ? integer + UINT32 : integer;
  }

  /** Names sorted by code unit, as every IndexedDB name list is. */
  function sortedNames(iterable) {
    const names = [...iterable];
    names.sort((a, b) => (a < b ? -1 : a > b ? 1 : 0));
    return names;
  }
