/**
 * The Presentation API as Chrome reports it with no cast device in reach. Chrome's shapes made
 * `PresentationRequest` exist with an illegal constructor, so Google's Cast sender (loaded by
 * YouTube's player for its remote-playback button) threw while starting. A request now
 * constructs from its URLs, `getAvailability()` resolves to an availability whose `value` is
 * false, and `start()`/`reconnect()` reject with NotFoundError, as Chrome does when no screen is
 * found. Mars has no presentation displays, so nothing here ever connects.
 */
(function installPresentation(global) {
  'use strict';
  const Request = global.PresentationRequest;
  const Availability = global.PresentationAvailability;
  const EventTargetCtor = global.EventTarget;
  if (typeof Request !== 'function' || typeof Availability !== 'function' || typeof EventTargetCtor !== 'function') return;
  const proto = Request.prototype;
  /** Request instances to their absolute URLs. */
  const urlsOf = new WeakMap();
  /** A non-enumerable own data property, as WebIDL operations are. */
  function define(target, name, value) {
    Object.defineProperty(target, name, { value, writable: true, enumerable: false, configurable: true });
  }
  /** A native-branded EventTarget wearing `prototype`. */
  function eventTarget(prototype) {
    const target = new EventTargetCtor();
    Object.setPrototypeOf(target, prototype);
    return target;
  }
  /** The absolute URLs a request was made with; a SyntaxError for an empty list or a bad URL. */
  function absolute(urls) {
    const list = typeof urls === 'string' ? [urls] : Array.from(urls || []);
    if (list.length === 0) throw new DOMException('An empty sequence of URLs is not supported.', 'NotSupportedError');
    return list.map((url) => {
      try {
        return new URL(String(url), global.location.href).href;
      } catch (error) {
        throw new DOMException(`'${url}' can't be resolved to a valid URL.`, 'SyntaxError');
      }
    });
  }
  function PresentationRequest(urls) {
    if (new.target === undefined) {
      throw new TypeError("Failed to construct 'PresentationRequest': Please use the 'new' operator, this DOM object constructor cannot be called as a function.");
    }
    const request = eventTarget(new.target.prototype || proto);
    urlsOf.set(request, absolute(urls));
    return request;
  }
  PresentationRequest.prototype = proto;
  define(proto, 'constructor', PresentationRequest);
  /** No screens: a found-nothing rejection, after the receiver check. */
  function noScreen(self, name) {
    if (!urlsOf.has(self)) return Promise.reject(new TypeError('Illegal invocation'));
    return Promise.reject(new DOMException(`Failed to execute '${name}' on 'PresentationRequest': No screens found.`, 'NotFoundError'));
  }
  define(proto, 'start', function start() { return noScreen(this, 'start'); });
  define(proto, 'reconnect', function reconnect() { return noScreen(this, 'reconnect'); });
  define(proto, 'getAvailability', function getAvailability() {
    if (!urlsOf.has(this)) return Promise.reject(new TypeError('Illegal invocation'));
    const availability = eventTarget(Availability.prototype);
    Object.defineProperty(availability, 'value', { value: false, enumerable: true });
    return Promise.resolve(availability);
  });
  define(global, 'PresentationRequest', PresentationRequest);
})(globalThis);
