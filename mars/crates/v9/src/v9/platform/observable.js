/**
 * Observable, Subscriber, EventTarget.prototype.when and the Observable operators, as the WICG
 * Observable proposal specifies them. The engine runs this once for each fresh page, before any
 * page script, as retained dynamic code, so the interfaces are ordinary globals whose closures
 * keep their source. Subscriber state lives in WeakMaps, never on the objects scripts can see, so
 * a page cannot reach another subscription's state. Only the names installed at the end are
 * visible on the global object; everything else stays inside this closure.
 */
(function installObservable(global) {
  'use strict';

  /** Subscriber object to the state behind it. */
  const stateOf = new WeakMap();
  /** Internal AbortSignal to the state that owns it, so a link can tell its owner apart from a page signal. */
  const ownerOf = new WeakMap();
  /** Values an error reports when its stack names no frame. */
  const NO_POSITION = 0;
  /** A `line:column` frame at the end of a stack line; the first match is the innermost frame. */
  const FRAME_POSITION = /:(\d+):(\d+)\)?$/m;
  /** Observer members a subscribe() call reads, in the order the proposal reads them. */
  const OBSERVER_KEYS = ['next', 'error', 'complete'];
  /** Inspector members inspect() reads; a bare function is the next handler. */
  const INSPECTOR_KEYS = ['subscribe', 'next', 'error', 'complete', 'abort'];
  /** Promise constructors captured now, so page code that replaces them cannot change conversions. */
  const resolveAsync = Promise.resolve.bind(Promise);

  /** Throw a TypeError unless the value is callable; returns the value. */
  function requireFunction(value, what) {
    if (typeof value !== 'function') {
      throw new TypeError(`${what} must be a function`);
    }
    return value;
  }

  /** Whether a value is an object or a function. */
  function isObjectLike(value) {
    return (typeof value === 'object' && value !== null) || typeof value === 'function';
  }

  /** The line and column of the innermost frame an error recorded, or zeros when it has none. */
  function positionOf(error) {
    const stack = isObjectLike(error) && typeof error.stack === 'string' ? error.stack : '';
    const match = FRAME_POSITION.exec(stack);
    return match ? [Number(match[1]), Number(match[2])] : [NO_POSITION, NO_POSITION];
  }

  /** Report an exception to the global: an error event, then the console unless a listener cancelled it. */
  function report(error) {
    const [lineno, colno] = positionOf(error);
    const event = new global.ErrorEvent('error', { cancelable: true });
    const message = isObjectLike(error) && typeof error.message === 'string' ? error.message : String(error);
    Object.defineProperties(event, {
      message: { value: message },
      error: { value: error },
      lineno: { value: lineno },
      colno: { value: colno },
    });
    if (global.dispatchEvent(event)) {
      global.console.error(error);
    }
  }

  /** Run a callback that must not throw out of the caller; its exception is reported. */
  function runGuarded(callback) {
    try {
      callback();
    } catch (error) {
      report(error);
    }
  }

  /** Read the callbacks of an observer or inspector: a bare function is the next handler. */
  function readCallbacks(source, keys) {
    const callbacks = { target: undefined };
    if (source === undefined || source === null) {
      return callbacks;
    }
    if (typeof source === 'function') {
      callbacks.next = source;
      return callbacks;
    }
    if (typeof source !== 'object') {
      throw new TypeError('An observer must be an object or a function');
    }
    callbacks.target = source;
    for (const key of keys) {
      const value = source[key];
      if (value !== undefined) {
        callbacks[key] = requireFunction(value, `observer ${key}`);
      }
    }
    return callbacks;
  }

  /** Call a callback if the observer has one; a throw is reported. Returns whether one existed. */
  function invoke(callbacks, key, args) {
    const callback = callbacks[key];
    if (callback === undefined) {
      return false;
    }
    try {
      callback.apply(callbacks.target, args);
    } catch (error) {
      report(error);
    }
    return true;
  }

  /** Call a callback if present and let its throw reach the caller. */
  function invokeOrThrow(callbacks, key, args) {
    const callback = callbacks[key];
    if (callback !== undefined) {
      callback.apply(callbacks.target, args);
    }
  }

  /** A fresh subscription: its own AbortController, abort algorithms, teardowns and observer. */
  function newState(observer) {
    const controller = new AbortController();
    const state = {
      controller,
      observer,
      closing: false,
      reason: undefined,
      algorithms: new Set(),
      teardowns: [],
    };
    ownerOf.set(controller.signal, state);
    return state;
  }

  /** The Subscriber a state belongs to, or a TypeError for a call made on anything else. */
  function stateFor(subscriber) {
    const state = stateOf.get(subscriber);
    if (state === undefined) {
      throw new TypeError('Illegal invocation: not a Subscriber');
    }
    return state;
  }

  /** Whether a subscriber can still deliver values. */
  function live(subscriber) {
    return !stateFor(subscriber).closing;
  }

  /**
   * Close a subscription: run its abort algorithms in insertion order, abort its signal (which
   * fires the abort event), then run its teardowns last-in first-out. The order is the proposal's.
   */
  function close(state, reason) {
    if (state.closing) {
      return;
    }
    state.closing = true;
    state.reason = reason;
    for (const algorithm of [...state.algorithms]) {
      algorithm(reason);
    }
    state.algorithms.clear();
    state.controller.abort(reason);
    while (state.teardowns.length > 0) {
      runGuarded(state.teardowns.pop());
    }
  }

  /** Register an abort algorithm on a subscriber, which runs before its abort event. */
  function onClose(subscriber, algorithm) {
    stateFor(subscriber).algorithms.add(algorithm);
  }

  /** Close a child when the signal it was given aborts, using the owner's algorithm list when it has one. */
  function link(child, signal) {
    const owner = ownerOf.get(signal);
    if (owner === undefined) {
      linkPageSignal(child, signal);
      return;
    }
    if (owner.closing) {
      close(child, owner.reason);
      return;
    }
    const algorithm = (reason) => close(child, reason);
    owner.algorithms.add(algorithm);
    child.teardowns.push(() => owner.algorithms.delete(algorithm));
  }

  /** Link a child to an AbortSignal that is not one of ours: an abort listener, removed on close. */
  function linkPageSignal(child, signal) {
    if (signal.aborted) {
      close(child, signal.reason);
      return;
    }
    const onAbort = () => close(child, signal.reason);
    signal.addEventListener('abort', onAbort, { once: true });
    child.teardowns.push(() => signal.removeEventListener('abort', onAbort));
  }

  /** Subscriber.prototype: the methods a subscriber exposes; each refuses a detached receiver. */
  const SUBSCRIBER_METHODS = {
    next(value) {
      if (arguments.length < 1) {
        throw new TypeError('Subscriber next() requires a value');
      }
      const state = stateFor(this);
      if (!state.closing) {
        invoke(state.observer, 'next', [value]);
      }
    },
    error(error) {
      if (arguments.length < 1) {
        throw new TypeError('Subscriber error() requires an error');
      }
      const state = stateFor(this);
      if (state.closing) {
        report(error);
        return;
      }
      close(state, error);
      if (!invoke(state.observer, 'error', [error])) {
        report(error);
      }
    },
    complete() {
      const state = stateFor(this);
      if (state.closing) {
        return;
      }
      close(state, undefined);
      invoke(state.observer, 'complete', []);
    },
    addTeardown(teardown) {
      requireFunction(teardown, 'teardown');
      const state = stateFor(this);
      if (state.closing) {
        runGuarded(teardown);
      } else {
        state.teardowns.push(teardown);
      }
    },
  };

  /** Subscriber constructor: scripts never construct one; the engine hands them out. */
  function Subscriber() {
    throw new TypeError('Subscriber is not constructible');
  }
  for (const key of Reflect.ownKeys(SUBSCRIBER_METHODS)) {
    Object.defineProperty(Subscriber.prototype, key, {
      value: SUBSCRIBER_METHODS[key],
      writable: true,
      enumerable: false,
      configurable: true,
    });
  }
  Object.defineProperties(Subscriber.prototype, {
    active: { get() { return live(this); }, enumerable: true, configurable: true },
    signal: { get() { return stateFor(this).controller.signal; }, enumerable: true, configurable: true },
  });

  /** Run an initializer under a new subscription to the observer, with the signal's link if given. */
  function subscribeWith(initializer, observerArg, options) {
    const state = newState(readCallbacks(observerArg, OBSERVER_KEYS));
    const signal = options === undefined || options === null ? undefined : options.signal;
    if (signal !== undefined) {
      link(state, signal);
    }
    const subscriber = Object.create(Subscriber.prototype);
    stateOf.set(subscriber, state);
    try {
      initializer(subscriber);
    } catch (error) {
      subscriber.error(error);
    }
  }

  /** Read a method that may be absent; a null or undefined member counts as absent. */
  function methodOf(input, key) {
    const method = input[key];
    if (method === undefined || method === null) {
      return undefined;
    }
    return requireFunction(method, `${String(key)} method`);
  }

  /** Close an iterator that stopped early, reporting a return() that fails. */
  function returnIterator(iterator) {
    const callback = iterator.return;
    if (callback === undefined || callback === null) {
      return;
    }
    const result = callback.call(iterator);
    if (!isObjectLike(result)) {
      throw new TypeError('Iterator return() must return an object');
    }
  }

  /** Emit each value of a synchronous iterable; an early stop calls its return(). */
  function iterateSync(input, method, subscriber) {
    const iterator = method.call(input);
    if (!isObjectLike(iterator)) {
      throw new TypeError('Iterator must be an object');
    }
    const record = { finished: false };
    subscriber.addTeardown(() => {
      if (!record.finished) {
        record.finished = true;
        returnIterator(iterator);
      }
    });
    while (live(subscriber)) {
      let step;
      try {
        step = iterator.next();
      } catch (error) {
        record.finished = true;
        throw error;
      }
      if (!isObjectLike(step)) {
        record.finished = true;
        throw new TypeError('Iterator result must be an object');
      }
      if (step.done) {
        record.finished = true;
        subscriber.complete();
        return;
      }
      subscriber.next(step.value);
    }
  }

  /** Call an async iterator's return() when it stops early; a failure is reported, never thrown. */
  function returnAsync(iterator) {
    let pending;
    try {
      const callback = iterator.return;
      if (callback === undefined || callback === null) {
        return;
      }
      pending = callback.call(iterator);
    } catch (error) {
      pending = Promise.reject(error);
    }
    resolveAsync(pending).then(undefined, report);
  }

  /** Emit each value of an async iterable, one next() at a time; results after a close are dropped. */
  function iterateAsync(input, method, subscriber) {
    const iterator = method.call(input);
    if (!isObjectLike(iterator)) {
      throw new TypeError('Async iterator must be an object');
    }
    const record = { finished: false };
    subscriber.addTeardown(() => {
      if (!record.finished) {
        record.finished = true;
        returnAsync(iterator);
      }
    });
    const pump = () => {
      if (!live(subscriber)) {
        return;
      }
      let pending;
      try {
        pending = resolveAsync(iterator.next());
      } catch (error) {
        pending = Promise.reject(error);
      }
      pending.then((step) => onStep(step), (error) => onFailure(error));
    };
    const onStep = (step) => {
      if (!live(subscriber)) {
        return;
      }
      if (!isObjectLike(step)) {
        onFailure(new TypeError('Async iterator result must be an object'));
        return;
      }
      if (step.done) {
        record.finished = true;
        subscriber.complete();
        return;
      }
      subscriber.next(step.value);
      pump();
    };
    const onFailure = (error) => {
      record.finished = true;
      if (live(subscriber)) {
        subscriber.error(error);
      }
    };
    pump();
  }

  /** A promise or thenable as an Observable: one value, then complete; a rejection is an error. */
  function fromThenable(thenable) {
    return new Observable((subscriber) => {
      resolveAsync(thenable).then(
        (value) => {
          if (live(subscriber)) {
            subscriber.next(value);
            subscriber.complete();
          }
        },
        (error) => {
          if (live(subscriber)) {
            subscriber.error(error);
          }
        },
      );
    });
  }

  /** Convert an Observable, async iterable, iterable or thenable to an Observable. */
  function convert(input) {
    if (input instanceof Observable) {
      return input;
    }
    if (input === undefined || input === null) {
      throw new TypeError('Value is neither an Observable, an iterable nor a Promise');
    }
    const asyncMethod = methodOf(input, Symbol.asyncIterator);
    if (asyncMethod !== undefined) {
      return new Observable((subscriber) => iterateAsync(input, asyncMethod, subscriber));
    }
    const syncMethod = methodOf(input, Symbol.iterator);
    if (syncMethod !== undefined) {
      return new Observable((subscriber) => iterateSync(input, syncMethod, subscriber));
    }
    if (isObjectLike(input) && typeof input.then === 'function') {
      return fromThenable(input);
    }
    throw new TypeError('Value is neither an Observable, an iterable nor a Promise');
  }

  /**
   * Run a promise-returning operator: the promise settles with the first value its start
   * functions pass to end.value() or end.fail(), and settling unsubscribes from the source. An
   * aborted signal rejects with its reason, after its own listeners have run.
   */
  function consumePromise(source, options, start) {
    return new Promise((resolve, reject) => {
      const signal = options === undefined || options === null ? undefined : options.signal;
      if (signal !== undefined && signal.aborted) {
        reject(signal.reason);
        return;
      }
      const controller = new AbortController();
      let settled = false;
      const finish = (settle, value) => {
        if (settled) {
          return;
        }
        settled = true;
        if (signal !== undefined) {
          signal.removeEventListener('abort', onAbort);
        }
        controller.abort();
        settle(value);
      };
      const end = {
        value: (value) => finish(resolve, value),
        fail: (error) => finish(reject, error),
      };
      const onAbort = () => end.fail(signal.reason);
      if (signal !== undefined) {
        signal.addEventListener('abort', onAbort, { once: true });
      }
      source.subscribe({
        next: (value) => {
          try {
            if (!settled) start.next(value, end);
          } catch (error) {
            end.fail(error);
          }
        },
        error: (error) => end.fail(error),
        complete: () => {
          if (!settled) start.complete(end);
        },
      }, { signal: controller.signal });
    });
  }

  /** The error and complete handlers that pass an upstream's terminal events on to a subscriber. */
  function relayTerminal(subscriber) {
    return {
      error: (error) => subscriber.error(error),
      complete: () => subscriber.complete(),
    };
  }

  /** All three handlers that pass an upstream's events on to a subscriber unchanged. */
  function relayAll(subscriber) {
    return {
      next: (value) => subscriber.next(value),
      ...relayTerminal(subscriber),
    };
  }

  /** The Observable class: the constructor records an initializer, subscribe() runs it. */
  class Observable {
    #initializer;

    /** Build an Observable from its initializer; nothing runs until a subscribe() call. */
    constructor(initializer) {
      this.#initializer = requireFunction(initializer, 'Observable initializer');
    }

    /** Subscribe with an observer or a next function; the initializer runs synchronously. */
    subscribe(observer, options) {
      subscribeWith(this.#initializer, observer, options);
    }

    /** Convert an Observable, iterable, async iterable or Promise to an Observable. */
    static from(input) {
      return convert(input);
    }

    /** An Observable that emits each argument synchronously, then completes. */
    static of(...values) {
      return new Observable((subscriber) => {
        for (const value of values) {
          if (!live(subscriber)) {
            return;
          }
          subscriber.next(value);
        }
        subscriber.complete();
      });
    }

    /** Emit the mapper's result for each value; a throwing mapper errors the result. */
    map(mapper) {
      requireFunction(mapper, 'mapper');
      const source = this;
      return new Observable((subscriber) => {
        let index = 0;
        source.subscribe({
          next(value) {
            let mapped;
            try {
              mapped = mapper(value, index++);
            } catch (error) {
              subscriber.error(error);
              return;
            }
            subscriber.next(mapped);
          },
          ...relayTerminal(subscriber),
        }, { signal: subscriber.signal });
      });
    }

    /** Emit the values the predicate keeps. */
    filter(predicate) {
      requireFunction(predicate, 'predicate');
      const source = this;
      return new Observable((subscriber) => {
        let index = 0;
        source.subscribe({
          next(value) {
            let keep;
            try {
              keep = predicate(value, index++);
            } catch (error) {
              subscriber.error(error);
              return;
            }
            if (keep) subscriber.next(value);
          },
          ...relayTerminal(subscriber),
        }, { signal: subscriber.signal });
      });
    }

    /** Emit the first `count` values, then complete; a count of zero never subscribes. */
    take(count) {
      const limit = count >>> 0;
      const source = this;
      return new Observable((subscriber) => {
        if (limit === 0) {
          subscriber.complete();
          return;
        }
        let seen = 0;
        source.subscribe({
          next(value) {
            seen += 1;
            subscriber.next(value);
            if (seen >= limit) subscriber.complete();
          },
          ...relayTerminal(subscriber),
        }, { signal: subscriber.signal });
      });
    }

    /** Skip the first `count` values, then emit the rest. */
    drop(count) {
      const limit = count >>> 0;
      const source = this;
      return new Observable((subscriber) => {
        let seen = 0;
        source.subscribe({
          next(value) {
            if (seen < limit) {
              seen += 1;
              return;
            }
            subscriber.next(value);
          },
          ...relayTerminal(subscriber),
        }, { signal: subscriber.signal });
      });
    }

    /** Subscribe to each mapped inner Observable one at a time, in order (concatMap). */
    flatMap(mapper) {
      requireFunction(mapper, 'mapper');
      const source = this;
      return new Observable((subscriber) => {
        const queue = [];
        let head = 0;
        let innerActive = false;
        let sourceDone = false;
        let index = 0;
        const subscribeInner = (inner) => {
          innerActive = true;
          inner.subscribe({
            next: (value) => subscriber.next(value),
            error: (error) => subscriber.error(error),
            complete: () => {
              innerActive = false;
              drain();
            },
          }, { signal: subscriber.signal });
        };
        const drain = () => {
          if (innerActive) return;
          if (head < queue.length) {
            const inner = queue[head];
            queue[head] = undefined;
            head += 1;
            subscribeInner(inner);
          } else if (sourceDone) {
            subscriber.complete();
          }
        };
        source.subscribe({
          next(value) {
            let inner;
            try {
              inner = convert(mapper(value, index++));
            } catch (error) {
              subscriber.error(error);
              return;
            }
            queue.push(inner);
            drain();
          },
          error: (error) => subscriber.error(error),
          complete() {
            sourceDone = true;
            drain();
          },
        }, { signal: subscriber.signal });
      });
    }

    /** Subscribe to each mapped inner Observable, unsubscribing from the previous one. */
    switchMap(mapper) {
      requireFunction(mapper, 'mapper');
      const source = this;
      return new Observable((subscriber) => {
        let current;
        let innerActive = false;
        let outerDone = false;
        let index = 0;
        onClose(subscriber, () => {
          if (current !== undefined) current.abort();
        });
        source.subscribe({
          next(value) {
            let inner;
            try {
              inner = convert(mapper(value, index++));
            } catch (error) {
              subscriber.error(error);
              return;
            }
            if (current !== undefined) current.abort();
            const attempt = new AbortController();
            current = attempt;
            innerActive = true;
            inner.subscribe({
              next: (item) => subscriber.next(item),
              error: (error) => subscriber.error(error),
              complete: () => {
                if (current !== attempt) return;
                current = undefined;
                innerActive = false;
                if (outerDone) subscriber.complete();
              },
            }, { signal: attempt.signal });
          },
          error: (error) => subscriber.error(error),
          complete() {
            outerDone = true;
            if (!innerActive) subscriber.complete();
          },
        }, { signal: subscriber.signal });
      });
    }

    /** Mirror the source until the notifier emits, which completes the result. */
    takeUntil(notifier) {
      const source = this;
      const stopper = convert(notifier);
      return new Observable((subscriber) => {
        stopper.subscribe({
          next: () => subscriber.complete(),
          error: (error) => subscriber.error(error),
        }, { signal: subscriber.signal });
        if (!live(subscriber)) return;
        source.subscribe(relayAll(subscriber), { signal: subscriber.signal });
      });
    }

    /** Recover from an error by subscribing to the Observable the handler returns. */
    catch(handler) {
      requireFunction(handler, 'handler');
      const source = this;
      return new Observable((subscriber) => {
        source.subscribe({
          next: (value) => subscriber.next(value),
          error(error) {
            let replacement;
            try {
              replacement = convert(handler(error));
            } catch (failure) {
              subscriber.error(failure);
              return;
            }
            replacement.subscribe(relayAll(subscriber), { signal: subscriber.signal });
          },
          complete: () => subscriber.complete(),
        }, { signal: subscriber.signal });
      });
    }

    /** Run the callback once when the result completes, errors, or is unsubscribed from. */
    finally(callback) {
      requireFunction(callback, 'callback');
      const source = this;
      return new Observable((subscriber) => {
        let ran = false;
        const runOnce = () => {
          if (ran) return;
          ran = true;
          runGuarded(callback);
        };
        onClose(subscriber, runOnce);
        source.subscribe({
          next: (value) => subscriber.next(value),
          error(error) {
            runOnce();
            subscriber.error(error);
          },
          complete() {
            runOnce();
            subscriber.complete();
          },
        }, { signal: subscriber.signal });
      });
    }

    /** Give the inspector each subscription step, then pass everything through unchanged. */
    inspect(inspector) {
      const callbacks = readCallbacks(inspector, INSPECTOR_KEYS);
      const source = this;
      return new Observable((subscriber) => {
        let finished = false;
        onClose(subscriber, (reason) => {
          if (!finished) invoke(callbacks, 'abort', [reason]);
        });
        try {
          invokeOrThrow(callbacks, 'subscribe', []);
        } catch (error) {
          subscriber.error(error);
          return;
        }
        source.subscribe({
          next(value) {
            try {
              invokeOrThrow(callbacks, 'next', [value]);
            } catch (error) {
              subscriber.error(error);
              return;
            }
            subscriber.next(value);
          },
          error(error) {
            finished = true;
            try {
              invokeOrThrow(callbacks, 'error', [error]);
            } catch (failure) {
              subscriber.error(failure);
              return;
            }
            subscriber.error(error);
          },
          complete() {
            finished = true;
            try {
              invokeOrThrow(callbacks, 'complete', []);
            } catch (failure) {
              subscriber.error(failure);
              return;
            }
            subscriber.complete();
          },
        }, { signal: subscriber.signal });
      });
    }

    /** Resolve with every value once the source completes. */
    toArray(options) {
      const values = [];
      return consumePromise(this, options, {
        next: (value) => values.push(value),
        complete: (end) => end.value(values),
      });
    }

    /** Call the visitor for each value; a throw rejects and unsubscribes. */
    forEach(visitor, options) {
      requireFunction(visitor, 'visitor');
      let index = 0;
      return consumePromise(this, options, {
        next: (value, end) => {
          try {
            visitor(value, index++);
          } catch (error) {
            end.fail(error);
          }
        },
        complete: (end) => end.value(undefined),
      });
    }

    /** Fold the values with the reducer; without a seed the first value seeds it. */
    reduce(reducer, seed, options) {
      requireFunction(reducer, 'reducer');
      const hasSeed = arguments.length >= 2;
      let accumulator = seed;
      let seeded = hasSeed;
      let index = 0;
      return consumePromise(this, options, {
        next: (value) => {
          const position = index++;
          if (!seeded) {
            accumulator = value;
            seeded = true;
            return;
          }
          accumulator = reducer(accumulator, value, position);
        },
        complete: (end) => {
          if (seeded) {
            end.value(accumulator);
          } else {
            end.fail(new TypeError('reduce() of an empty Observable needs a seed'));
          }
        },
      });
    }

    /** Resolve with the first value. */
    first(options) {
      return consumePromise(this, options, {
        next: (value, end) => end.value(value),
        complete: (end) => end.fail(new RangeError('first(): the Observable completed without a value')),
      });
    }

    /** Resolve with the last value. */
    last(options) {
      let latest;
      let any = false;
      return consumePromise(this, options, {
        next: (value) => {
          latest = value;
          any = true;
        },
        complete: (end) => {
          if (any) end.value(latest);
          else end.fail(new RangeError('last(): the Observable completed without a value'));
        },
      });
    }

    /** Resolve true at the first value the predicate accepts, false if none does. */
    some(predicate, options) {
      requireFunction(predicate, 'predicate');
      let index = 0;
      return consumePromise(this, options, {
        next: (value, end) => {
          if (predicate(value, index++)) end.value(true);
        },
        complete: (end) => end.value(false),
      });
    }

    /** Resolve false at the first value the predicate rejects, true if all pass. */
    every(predicate, options) {
      requireFunction(predicate, 'predicate');
      let index = 0;
      return consumePromise(this, options, {
        next: (value, end) => {
          if (!predicate(value, index++)) end.value(false);
        },
        complete: (end) => end.value(true),
      });
    }

    /** Resolve with the first value the predicate accepts, or undefined. */
    find(predicate, options) {
      requireFunction(predicate, 'predicate');
      let index = 0;
      return consumePromise(this, options, {
        next: (value, end) => {
          if (predicate(value, index++)) end.value(value);
        },
        complete: (end) => end.value(undefined),
      });
    }
  }

  /** EventTarget.prototype.when: an Observable of the target's events, listening while subscribed. */
  function when(type, options) {
    const target = this;
    const capture = typeof options === 'boolean' ? options : Boolean(options && options.capture);
    return new Observable((subscriber) => {
      const listener = (event) => subscriber.next(event);
      target.addEventListener(type, listener, options);
      subscriber.addTeardown(() => target.removeEventListener(type, listener, capture));
    });
  }

  /** Install a global as an ordinary writable, configurable, non-enumerable property. */
  function define(target, name, value) {
    Object.defineProperty(target, name, { value, writable: true, enumerable: false, configurable: true });
  }

  define(global, 'Observable', Observable);
  define(global, 'Subscriber', Subscriber);
  // Sessions without the DOM (the conformance harness) have no EventTarget; `when` belongs only where it exists.
  if (typeof global.EventTarget === 'function') {
    define(global.EventTarget.prototype, 'when', when);
  }
})(globalThis);
