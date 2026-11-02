/**
 * View Transitions without the animation. Chrome's shapes made `document.startViewTransition`
 * exist but return undefined, so YouTube's navigation (`transition.skipTransition()`) threw on
 * every page change. Mars renders no transition pseudo-elements, which is the spec's "skip the
 * view transition" outcome: the update callback still runs, `updateCallbackDone` and `finished`
 * settle with it, and `ready` rejects with an AbortError, exactly as Chrome reports a skipped
 * transition. Instances are real ViewTransition objects; their state lives in a WeakMap.
 */
(function installViewTransitions(global) {
  'use strict';
  const document = global.document;
  const ViewTransition = global.ViewTransition;
  if (!document || typeof ViewTransition !== 'function') return;
  const proto = ViewTransition.prototype;
  /** Instance to its promises and types. */
  const slots = new WeakMap();
  /** The fields of a transition, or a TypeError for a foreign receiver. */
  function own(self) {
    const value = slots.get(self);
    if (!value) throw new TypeError('Illegal invocation');
    return value;
  }
  /** A non-enumerable own data property, as WebIDL operations are. */
  function define(target, name, value) {
    Object.defineProperty(target, name, { value, writable: true, enumerable: false, configurable: true });
  }
  for (const name of ['updateCallbackDone', 'ready', 'finished', 'types']) {
    Object.defineProperty(proto, name, { get() { return own(this)[name]; }, enumerable: true, configurable: true });
  }
  Object.defineProperty(proto, 'transitionRoot', { get() { own(this); return document.documentElement; }, enumerable: true, configurable: true });
  define(proto, 'skipTransition', function skipTransition() { own(this); });
  define(proto, 'waitUntil', function waitUntil() { own(this); });

  /** A skipped transition: the callback runs as a microtask, `ready` rejects with AbortError. */
  function startViewTransition(options) {
    const update = typeof options === 'function' ? options : options && options.update;
    const types = new Set(options && typeof options === 'object' && options.types ? options.types : []);
    const updateCallbackDone = Promise.resolve().then(() => (typeof update === 'function' ? update() : undefined)).then(() => undefined);
    const ready = Promise.reject(new DOMException('Transition was skipped', 'AbortError'));
    ready.catch(() => {});
    const finished = updateCallbackDone.then(() => undefined);
    finished.catch(() => {});
    const transition = Object.create(proto);
    slots.set(transition, { updateCallbackDone, ready, finished, types });
    return transition;
  }
  define((global.Document || Object.getPrototypeOf(document).constructor).prototype, 'startViewTransition', startViewTransition);
})(globalThis);
