
/**
 * HTML media elements, their tracks and Media Source, per the WHATWG HTML "media elements"
 * section and the Media Source Extensions specification. The script owns the element state
 * machine (resource selection, ready and network states, play and pause, seeking, events and
 * their order); the coordinator owns the actual player (fetching through Relay, decoding,
 * audio output, frames) and talks to this script only through `host`: `post` sends a command,
 * `listen` receives updates and changed elements, `support` lists what the decoders play and
 * `activation` says whether a person interacted with the page (Chrome's autoplay policy).
 * This file's completion value is its installer; the bootstrap calls it with the private host.
 */
(function installMedia(host) {
  'use strict';
  const global = globalThis;
  if (!global.document || typeof global.HTMLMediaElement !== 'function') return;
  /**
   * The implementation is built on first use: most pages have no media, and every object it
   * creates would otherwise live in every page's heap. Until then the interfaces keep their
   * inert shapes, whose members ask the host to build it (`ensure`) before answering.
   */
  let api = null;
  function ensure() {
    if (!api) api = build();
    return api;
  }

  function build() {

  // Natives captured before any page script can replace them.
  const ElementProto = global.Element.prototype;
  const NodeProto = global.Node.prototype;
  const getAttribute = ElementProto.getAttribute;
  const hasAttribute = ElementProto.hasAttribute;
  const setAttribute = ElementProto.setAttribute;
  const removeAttribute = ElementProto.removeAttribute;
  const localNameOf = Object.getOwnPropertyDescriptor(ElementProto, 'localName').get;
  const childrenOf = Object.getOwnPropertyDescriptor(NodeProto, 'childNodes').get;
  const parentOf = Object.getOwnPropertyDescriptor(NodeProto, 'parentNode').get;
  const connectedOf = Object.getOwnPropertyDescriptor(NodeProto, 'isConnected').get;
  const dispatch = global.EventTarget.prototype.dispatchEvent;
  const EventCtor = global.Event;
  const setTimer = global.setTimeout;
  const queueMicrotask = global.queueMicrotask;
  const now = () => global.performance.now();
  const baseURI = () => global.document.baseURI;
  const nativeFetch = global.fetch;
  const matchMedia = global.matchMedia;
  const createObjectURL = global.URL.createObjectURL;
  const revokeObjectURL = global.URL.revokeObjectURL;
  const randomUUID = global.crypto && global.crypto.randomUUID ? global.crypto.randomUUID.bind(global.crypto) : null;
  const support = parseSupport(host.support());

  /** Spec constants. */
  const NETWORK_EMPTY = 0, NETWORK_IDLE = 1, NETWORK_LOADING = 2, NETWORK_NO_SOURCE = 3;
  const HAVE_NOTHING = 0, HAVE_METADATA = 1, HAVE_CURRENT_DATA = 2, HAVE_FUTURE_DATA = 3, HAVE_ENOUGH_DATA = 4;
  const MEDIA_ERR_ABORTED = 1, MEDIA_ERR_NETWORK = 2, MEDIA_ERR_DECODE = 3, MEDIA_ERR_SRC_NOT_SUPPORTED = 4;
  /** Progress fires at most this often while fetching (HTML: "about every 350ms"). */
  const PROGRESS_MS = 350;
  /** Chrome's accepted playbackRate range; outside it, setting throws NotSupportedError. */
  const MIN_RATE = 0.0625, MAX_RATE = 16;
  /** Private token: interface objects the page sees are not constructible by it. */
  const KEY = {};

  function parseSupport(text) {
    try {
      const value = JSON.parse(text || '{}');
      return { containers: value.containers || [], codecs: value.codecs || [], streams: value.streams || [] };
    } catch (e) {
      return { containers: [], codecs: [], streams: [] };
    }
  }
  /** A non-enumerable, configurable, writable data property, as a platform installs one. */
  function define(target, name, value) {
    Object.defineProperty(target, name, { value, writable: true, enumerable: false, configurable: true });
  }
  /** WebIDL attributes and operations on a prototype: enumerable and configurable. */
  function idl(proto, members) {
    for (const name of Object.keys(members)) {
      const d = Object.getOwnPropertyDescriptor(members, name);
      if (typeof d.value === 'function') {
        Object.defineProperty(proto, name, { value: d.value, writable: true, enumerable: true, configurable: true });
      } else {
        Object.defineProperty(proto, name, { get: d.get, set: d.set, enumerable: true, configurable: true });
      }
    }
  }
  /** Replace a global interface with a script class, keeping Chrome's attribute shape. */
  function expose(name, cls) {
    define(global, name, cls);
    Object.defineProperty(cls.prototype, Symbol.toStringTag, { value: name, configurable: true });
  }
  function domError(name, message) {
    return new global.DOMException(message, name);
  }
  function illegal() {
    throw new TypeError('Illegal constructor');
  }
  /** Fire a simple event: not bubbling, not cancelable, as every media event is. */
  function fire(target, type, init) {
    const event = new EventCtor(type, init || {});
    if (init && init.track) Object.defineProperty(event, 'track', { value: init.track, enumerable: true, configurable: true });
    return dispatch.call(target, event);
  }

  // ---- Tasks: the media element event task source, in order, after the current task. ----
  const tasks = [];
  let flushing = false;
  function queueTask(work) {
    tasks.push(work);
    if (!flushing) {
      flushing = true;
      setTimer.call(global, flush, 0);
    }
  }
  function flush() {
    const batch = tasks.splice(0, tasks.length);
    flushing = false;
    for (const work of batch) {
      try { work(); } catch (e) { report(e); }
    }
    if (tasks.length && !flushing) {
      flushing = true;
      setTimer.call(global, flush, 0);
    }
  }
  function report(error) {
    if (typeof global.reportError === 'function') global.reportError(error);
  }
  function queueFire(target, type, init) {
    queueTask(() => fire(target, type, init));
  }

  // ---- TimeRanges and MediaError. ----
  const rangesOf = new WeakMap();
  class TimeRanges {
    constructor(token, ranges) {
      if (token !== KEY) illegal();
      rangesOf.set(this, normalize(ranges || []));
    }
    get length() { return ranges(this).length; }
    start(index) { return bound(this, index)[0]; }
    end(index) { return bound(this, index)[1]; }
  }
  function ranges(self) {
    const value = rangesOf.get(self);
    if (!value) throw new TypeError('Illegal invocation');
    return value;
  }
  function bound(self, index) {
    const list = ranges(self);
    index = Number(index) >>> 0;
    if (index >= list.length) {
      throw domError('IndexSizeError', `Failed to execute 'start' on 'TimeRanges': The index provided (${index}) is greater than or equal to the maximum bound (${list.length}).`);
    }
    return list[index];
  }
  /** Sorted, merged, non-empty ranges, as TimeRanges requires. */
  function normalize(list) {
    const sorted = list.filter(r => r && r[1] > r[0]).map(r => [Number(r[0]), Number(r[1])]).sort((a, b) => a[0] - b[0]);
    const out = [];
    for (const r of sorted) {
      const last = out[out.length - 1];
      if (last && r[0] <= last[1]) last[1] = Math.max(last[1], r[1]);
      else out.push(r);
    }
    return out;
  }
  function timeRanges(list) { return new TimeRanges(KEY, list); }
  expose('TimeRanges', TimeRanges);

  const errorOf = new WeakMap();
  class MediaError {
    constructor(token, code, message) {
      if (token !== KEY) illegal();
      errorOf.set(this, { code, message: message || '' });
    }
    get code() { return mediaError(this).code; }
    get message() { return mediaError(this).message; }
  }
  function mediaError(self) {
    const value = errorOf.get(self);
    if (!value) throw new TypeError('Illegal invocation');
    return value;
  }
  const ERROR_CODES = { MEDIA_ERR_ABORTED, MEDIA_ERR_NETWORK, MEDIA_ERR_DECODE, MEDIA_ERR_SRC_NOT_SUPPORTED };
  for (const name of Object.keys(ERROR_CODES)) {
    Object.defineProperty(MediaError, name, { value: ERROR_CODES[name], enumerable: true });
    Object.defineProperty(MediaError.prototype, name, { value: ERROR_CODES[name], enumerable: true });
  }
  expose('MediaError', MediaError);

  // ---- Audio and video tracks (HTML "AudioTrackList and VideoTrackList objects"). ----
  const trackOf = new WeakMap();
  class AudioTrack {
    constructor(token, info) {
      if (token !== KEY) illegal();
      trackOf.set(this, Object.assign({ enabled: true, list: null }, info));
    }
    get id() { return track(this).id; }
    get kind() { return track(this).kind; }
    get label() { return track(this).label; }
    get language() { return track(this).language; }
    get enabled() { return track(this).enabled; }
    set enabled(value) {
      const t = track(this);
      value = !!value;
      if (t.enabled === value) return;
      t.enabled = value;
      if (t.list) queueFire(t.list, 'change');
    }
  }
  class VideoTrack {
    constructor(token, info) {
      if (token !== KEY) illegal();
      trackOf.set(this, Object.assign({ selected: true, list: null }, info));
    }
    get id() { return track(this).id; }
    get kind() { return track(this).kind; }
    get label() { return track(this).label; }
    get language() { return track(this).language; }
    get selected() { return track(this).selected; }
    set selected(value) {
      const t = track(this);
      value = !!value;
      if (t.selected === value) return;
      if (value && t.list) for (const other of listItems(t.list)) if (other !== this) track(other).selected = false;
      t.selected = value;
      if (t.list) queueFire(t.list, 'change');
    }
  }
  function track(self) {
    const value = trackOf.get(self);
    if (!value) throw new TypeError('Illegal invocation');
    return value;
  }

  /** Lists keep their items here and mirror them as indexed own properties. */
  const itemsOf = new WeakMap();
  function listItems(list) {
    const value = itemsOf.get(list);
    if (!value) throw new TypeError('Illegal invocation');
    return value;
  }
  function setItems(list, items) {
    const old = itemsOf.get(list) || [];
    for (let i = items.length; i < old.length; i++) delete list[i];
    itemsOf.set(list, items);
    items.forEach((item, i) => Object.defineProperty(list, i, { value: item, enumerable: true, configurable: true }));
  }
  function trackListClass(name, extra) {
    const cls = class extends global.EventTarget {
      constructor(token) {
        if (token !== KEY) illegal();
        super();
        setItems(this, []);
      }
      get length() { return listItems(this).length; }
      getTrackById(id) {
        id = String(id);
        return listItems(this).find(t => t.id === id) || null;
      }
    };
    Object.defineProperty(cls, 'name', { value: name });
    handlers(cls.prototype, ['change', 'addtrack', 'removetrack']);
    if (extra) idl(cls.prototype, extra);
    cls.prototype[Symbol.iterator] = Array.prototype[Symbol.iterator];
    expose(name, cls);
    return cls;
  }
  /** `onX` event handler IDL attributes backed by a listener the attribute owns. */
  function handlers(proto, names) {
    const store = new WeakMap();
    for (const name of names) {
      Object.defineProperty(proto, 'on' + name, {
        get() { const own = store.get(this); return (own && own[name] && own[name].value) || null; },
        set(value) {
          let own = store.get(this);
          if (!own) store.set(this, own = {});
          if (!own[name]) {
            const slot = own[name] = { value: null };
            this.addEventListener(name, function (event) {
              if (typeof slot.value === 'function') return slot.value.call(this, event);
            });
          }
          own[name].value = typeof value === 'function' || (value && typeof value === 'object') ? value : null;
        },
        enumerable: true,
        configurable: true,
      });
    }
  }
  const AudioTrackList = trackListClass('AudioTrackList');
  const VideoTrackList = trackListClass('VideoTrackList', {
    get selectedIndex() { return listItems(this).findIndex(t => track(t).selected); },
  });
  expose('AudioTrack', AudioTrack);
  expose('VideoTrack', VideoTrack);

  // ---- Text tracks: TextTrack, cues, WebVTT (HTML "timed text tracks", WebVTT spec). ----
  const textOf = new WeakMap();
  class TextTrack extends global.EventTarget {
    constructor(token, info) {
      if (token !== KEY) illegal();
      super();
      textOf.set(this, {
        kind: info.kind, label: info.label || '', language: info.language || '', id: info.id || '',
        mode: info.mode || 'disabled', cues: [], active: [], media: info.media || null,
        element: info.element || null, readiness: info.element ? 'not loaded' : 'loaded',
        cueList: null, activeList: null,
      });
    }
    get kind() { return text(this).kind; }
    get label() { return text(this).label; }
    get language() { return text(this).language; }
    get id() { return text(this).id; }
    get inBandMetadataTrackDispatchType() { return ''; }
    get mode() { return text(this).mode; }
    set mode(value) {
      value = String(value);
      if (!['disabled', 'hidden', 'showing'].includes(value)) return;
      const t = text(this);
      if (t.mode === value) return;
      t.mode = value;
      if (value !== 'disabled' && t.element) loadTrack(t.element);
      if (t.media) {
        const list = state(t.media).textTracks;
        queueFire(list, 'change');
        timeMarchesOn(t.media);
      }
    }
    get cues() {
      const t = text(this);
      if (t.mode === 'disabled') return null;
      return t.cueList || (t.cueList = new TextTrackCueList(KEY, () => t.cues));
    }
    get activeCues() {
      const t = text(this);
      if (t.mode === 'disabled') return null;
      return t.activeList || (t.activeList = new TextTrackCueList(KEY, () => t.active));
    }
    addCue(cue) {
      if (!cueOf.has(cue)) throw new TypeError("Failed to execute 'addCue' on 'TextTrack': parameter 1 is not of type 'TextTrackCue'.");
      const c = cueOf.get(cue);
      if (c.track) c.track.removeCue(cue);
      c.track = this;
      const list = text(this).cues;
      list.push(cue);
      list.sort(cueOrder);
      if (text(this).media) timeMarchesOn(text(this).media);
    }
    removeCue(cue) {
      const c = cueOf.get(cue);
      const t = text(this);
      const at = t.cues.indexOf(cue);
      if (!c || at < 0) throw domError('NotFoundError', "Failed to execute 'removeCue' on 'TextTrack': The specified cue is not listed in the TextTrack's list of cues.");
      t.cues.splice(at, 1);
      c.track = null;
      if (t.media) timeMarchesOn(t.media);
    }
  }
  handlers(TextTrack.prototype, ['cuechange']);
  function text(self) {
    const value = textOf.get(self);
    if (!value) throw new TypeError('Illegal invocation');
    return value;
  }
  /** Cues sort by start time, then by end time descending, then by insertion. */
  function cueOrder(a, b) {
    const x = cueOf.get(a), y = cueOf.get(b);
    return x.startTime - y.startTime || y.endTime - x.endTime || x.order - y.order;
  }
  expose('TextTrack', TextTrack);

  const cueListSource = new WeakMap();
  class TextTrackCueList {
    constructor(token, source) {
      if (token !== KEY) illegal();
      cueListSource.set(this, source);
      return indexed(this, source, cueListSource, {
        get(target, key, receiver) {
          if (typeof key === 'string' && /^(0|[1-9][0-9]*)$/.test(key)) return source()[Number(key)];
          return Reflect.get(target, key, target);
        },
        has(target, key) {
          if (typeof key === 'string' && /^(0|[1-9][0-9]*)$/.test(key)) return Number(key) < source().length;
          return Reflect.has(target, key);
        },
      });
    }
    get length() { return cues(this).length; }
    getCueById(id) {
      id = String(id);
      return cues(this).find(c => cueOf.get(c).id === id) || null;
    }
    [Symbol.iterator]() { return cues(this).slice()[Symbol.iterator](); }
  }
  /** A live indexed view (`list[0]`) over a getter; the proxy shares the target's private state. */
  function indexed(target, getter, store, traps) {
    const proxy = new Proxy(target, traps);
    store.set(proxy, getter);
    return proxy;
  }
  function cues(self) {
    const source = cueListSource.get(self);
    if (!source) throw new TypeError('Illegal invocation');
    return source();
  }
  expose('TextTrackCueList', TextTrackCueList);

  const cueOf = new WeakMap();
  let cueSerial = 0;
  class TextTrackCue extends global.EventTarget {
    constructor(token) {
      if (token !== KEY) illegal();
      super();
    }
    get track() { return cue(this).track; }
    get id() { return cue(this).id; }
    set id(value) { cue(this).id = String(value); }
    get startTime() { return cue(this).startTime; }
    set startTime(value) { setCueTime(this, 'startTime', value); }
    get endTime() { return cue(this).endTime; }
    set endTime(value) { setCueTime(this, 'endTime', value); }
    get pauseOnExit() { return cue(this).pauseOnExit; }
    set pauseOnExit(value) { cue(this).pauseOnExit = !!value; }
  }
  handlers(TextTrackCue.prototype, ['enter', 'exit']);
  function cue(self) {
    const value = cueOf.get(self);
    if (!value) throw new TypeError('Illegal invocation');
    return value;
  }
  function finite(value, owner, member) {
    const number = Number(value);
    if (!Number.isFinite(number)) {
      throw new TypeError(`Failed to set the '${member}' property on '${owner}': The provided double value is non-finite.`);
    }
    return number;
  }
  function setCueTime(self, name, value) {
    const c = cue(self);
    c[name] = finite(value, 'TextTrackCue', name);
    if (c.track) {
      text(c.track).cues.sort(cueOrder);
      if (text(c.track).media) timeMarchesOn(text(c.track).media);
    }
  }
  expose('TextTrackCue', TextTrackCue);

  const DIRECTIONS = ['', 'rl', 'lr'];
  const ALIGNS = ['start', 'center', 'end', 'left', 'right'];
  const LINE_ALIGNS = ['start', 'center', 'end'];
  const POSITION_ALIGNS = ['line-left', 'center', 'line-right', 'auto'];
  class VTTCue extends TextTrackCue {
    constructor(startTime, endTime, text) {
      if (arguments.length < 3) {
        throw new TypeError(`Failed to construct 'VTTCue': 3 arguments required, but only ${arguments.length} present.`);
      }
      super(KEY);
      cueOf.set(this, {
        track: null, id: '', order: cueSerial++, pauseOnExit: false,
        startTime: finite(startTime, 'VTTCue', 'startTime'), endTime: finite(endTime, 'VTTCue', 'endTime'),
        text: String(text), vertical: '', snapToLines: true, line: 'auto', lineAlign: 'start',
        position: 'auto', positionAlign: 'auto', size: 100, align: 'center', region: null,
      });
    }
    get vertical() { return cue(this).vertical; }
    set vertical(value) { if (DIRECTIONS.includes(String(value))) cue(this).vertical = String(value); }
    get snapToLines() { return cue(this).snapToLines; }
    set snapToLines(value) { cue(this).snapToLines = !!value; }
    get line() { return cue(this).line; }
    set line(value) { cue(this).line = value === 'auto' ? 'auto' : finite(value, 'VTTCue', 'line'); }
    get lineAlign() { return cue(this).lineAlign; }
    set lineAlign(value) { if (LINE_ALIGNS.includes(String(value))) cue(this).lineAlign = String(value); }
    get position() { return cue(this).position; }
    set position(value) {
      if (value === 'auto') { cue(this).position = 'auto'; return; }
      const number = finite(value, 'VTTCue', 'position');
      if (number < 0 || number > 100) throw domError('IndexSizeError', `Failed to set the 'position' property on 'VTTCue': The value provided (${number}) is outside the range [0, 100].`);
      cue(this).position = number;
    }
    get positionAlign() { return cue(this).positionAlign; }
    set positionAlign(value) { if (POSITION_ALIGNS.includes(String(value))) cue(this).positionAlign = String(value); }
    get size() { return cue(this).size; }
    set size(value) {
      const number = finite(value, 'VTTCue', 'size');
      if (number < 0 || number > 100) throw domError('IndexSizeError', `Failed to set the 'size' property on 'VTTCue': The value provided (${number}) is outside the range [0, 100].`);
      cue(this).size = number;
    }
    get align() { return cue(this).align; }
    set align(value) { if (ALIGNS.includes(String(value))) cue(this).align = String(value); }
    get text() { return cue(this).text; }
    set text(value) { cue(this).text = String(value); }
    get region() { return cue(this).region; }
    set region(value) {
      if (value !== null && !regionOf.has(value)) throw new TypeError("Failed to set the 'region' property on 'VTTCue': The provided value is not of type 'VTTRegion'.");
      cue(this).region = value;
    }
    getCueAsHTML() { return cueFragment(cue(this).text); }
  }
  expose('VTTCue', VTTCue);

  const regionOf = new WeakMap();
  class VTTRegion {
    constructor() {
      regionOf.set(this, { id: '', width: 100, lines: 3, regionAnchorX: 0, regionAnchorY: 100, viewportAnchorX: 0, viewportAnchorY: 100, scroll: '' });
    }
  }
  for (const name of ['width', 'regionAnchorX', 'regionAnchorY', 'viewportAnchorX', 'viewportAnchorY']) {
    Object.defineProperty(VTTRegion.prototype, name, {
      get() { return region(this)[name]; },
      set(value) {
        const number = finite(value, 'VTTRegion', name);
        if (number < 0 || number > 100) throw domError('IndexSizeError', `Failed to set the '${name}' property on 'VTTRegion': The value provided (${number}) is outside the range [0, 100].`);
        region(this)[name] = number;
      },
      enumerable: true, configurable: true,
    });
  }
  idl(VTTRegion.prototype, {
    get id() { return region(this).id; },
    set id(value) { region(this).id = String(value); },
    get lines() { return region(this).lines; },
    set lines(value) { region(this).lines = Number(value) >>> 0; },
    get scroll() { return region(this).scroll; },
    set scroll(value) { if (value === '' || value === 'up') region(this).scroll = value; },
  });
  function region(self) {
    const value = regionOf.get(self);
    if (!value) throw new TypeError('Illegal invocation');
    return value;
  }
  expose('VTTRegion', VTTRegion);

  /** WebVTT cue text as DOM: c, i, b, u, ruby, rt, v and lang spans, per the cue text parser. */
  function cueFragment(source) {
    const doc = global.document;
    const fragment = doc.createDocumentFragment();
    const stack = [fragment];
    const TAGS = { c: 'span', i: 'i', b: 'b', u: 'u', ruby: 'ruby', rt: 'rt', v: 'span', lang: 'span' };
    const pattern = /<(\/?)([a-z]+|\d[\d:.]*)((?:\.[^\s>]*)?)(?:\s+([^>]*))?>|([^<]+)/g;
    let match;
    while ((match = pattern.exec(source))) {
      const top = stack[stack.length - 1];
      if (match[5] !== undefined) {
        top.appendChild(doc.createTextNode(decodeEntities(match[5])));
      } else if (match[1]) {
        if (stack.length > 1 && stack[stack.length - 1].__cueTag === match[2]) stack.pop();
      } else if (TAGS[match[2]]) {
        const node = doc.createElement(TAGS[match[2]]);
        const classes = match[3] ? match[3].slice(1).split('.').filter(Boolean) : [];
        if (classes.length) node.className = classes.join(' ');
        if (match[2] === 'v' && match[4]) node.title = match[4].trim();
        if (match[2] === 'lang' && match[4]) node.lang = match[4].trim();
        Object.defineProperty(node, '__cueTag', { value: match[2], configurable: true });
        top.appendChild(node);
        stack.push(node);
      }
    }
    return fragment;
  }
  function decodeEntities(text) {
    return text.replace(/&(amp|lt|gt|lrm|rlm|nbsp);/g, (_, name) =>
      ({ amp: '&', lt: '<', gt: '>', lrm: '‎', rlm: '‏', nbsp: ' ' })[name]);
  }
  /** Plain caption text: cue text with tags removed and entities decoded. */
  function cueText(source) {
    return decodeEntities(String(source).replace(/<[^>]*>/g, ''));
  }

  /** WebVTT parser (W3C WebVTT "file parsing"): regions and cues; styles and notes are skipped. */
  function parseWebVTT(input) {
    const lines = String(input).replace(/\r\n?/g, '\n').split('\n');
    if (!/^(﻿)?WEBVTT([ \t].*)?$/.test(lines[0])) throw new Error('Not a WebVTT file');
    const cuesOut = [], regions = new Map();
    let i = 1;
    while (i < lines.length && lines[i] !== '') i++;
    while (i < lines.length) {
      while (i < lines.length && lines[i] === '') i++;
      const block = [];
      while (i < lines.length && lines[i] !== '') block.push(lines[i++]);
      if (!block.length) continue;
      if (/^REGION([ \t]|$)/.test(block[0])) { const r = parseRegion(block.slice(1).join(' ')); if (r) regions.set(region(r).id, r); continue; }
      if (/^(NOTE|STYLE)([ \t]|$)/.test(block[0])) continue;
      let id = '';
      let at = 0;
      if (!block[0].includes('-->')) { id = block[0]; at = 1; }
      if (at >= block.length || !block[at].includes('-->')) continue;
      const timing = /^\s*([\d:.]+)[ \t]+-->[ \t]+([\d:.]+)(.*)$/.exec(block[at]);
      if (!timing) continue;
      const start = timestamp(timing[1]), end = timestamp(timing[2]);
      if (start === null || end === null) continue;
      const c = new VTTCue(start, end, block.slice(at + 1).join('\n'));
      c.id = id;
      cueSettings(c, timing[3], regions);
      cuesOut.push(c);
    }
    return cuesOut;
  }
  function timestamp(text) {
    const m = /^(?:(\d{2,}):)?([0-5]\d):([0-5]\d)\.(\d{3})$/.exec(text);
    if (!m) return null;
    return Number(m[1] || 0) * 3600 + Number(m[2]) * 60 + Number(m[3]) + Number(m[4]) / 1000;
  }
  function percent(text) {
    const m = /^(\d+(?:\.\d+)?)%$/.exec(text);
    return m ? Math.min(100, Number(m[1])) : null;
  }
  function cueSettings(c, text, regions) {
    for (const setting of text.trim().split(/[ \t]+/)) {
      const at = setting.indexOf(':');
      if (at <= 0) continue;
      const name = setting.slice(0, at), value = setting.slice(at + 1);
      try {
        if (name === 'vertical' && (value === 'rl' || value === 'lr')) c.vertical = value;
        else if (name === 'line') {
          const [line, align] = value.split(',');
          const p = percent(line);
          if (p !== null) { c.snapToLines = false; c.line = p; }
          else if (/^-?\d+$/.test(line)) { c.snapToLines = true; c.line = Number(line); }
          if (align) c.lineAlign = align;
        } else if (name === 'position') {
          const [position, align] = value.split(',');
          const p = percent(position);
          if (p !== null) c.position = p;
          if (align) c.positionAlign = align;
        } else if (name === 'size') { const p = percent(value); if (p !== null) c.size = p; }
        else if (name === 'align') c.align = value;
        else if (name === 'region' && regions.has(value)) c.region = regions.get(value);
      } catch (e) { /* An invalid setting is ignored, as the parser requires. */ }
    }
  }
  function parseRegion(text) {
    const r = new VTTRegion();
    for (const setting of text.trim().split(/[ \t]+/)) {
      const at = setting.indexOf(':');
      if (at <= 0) continue;
      const name = setting.slice(0, at), value = setting.slice(at + 1);
      try {
        if (name === 'id') r.id = value;
        else if (name === 'width') { const p = percent(value); if (p !== null) r.width = p; }
        else if (name === 'lines' && /^\d+$/.test(value)) r.lines = Number(value);
        else if (name === 'regionanchor' || name === 'viewportanchor') {
          const [x, y] = value.split(',').map(percent);
          if (x !== null && y !== null) {
            if (name === 'regionanchor') { r.regionAnchorX = x; r.regionAnchorY = y; }
            else { r.viewportAnchorX = x; r.viewportAnchorY = y; }
          }
        } else if (name === 'scroll' && value === 'up') r.scroll = 'up';
      } catch (e) { /* ignored */ }
    }
    return r.id ? r : null;
  }

  const TextTrackList = trackListClass('TextTrackList');

  // ---- HTMLTrackElement. ----
  const trackStates = new WeakMap();
  const TRACK_NONE = 0, TRACK_LOADING = 1, TRACK_LOADED = 2, TRACK_ERROR = 3;
  function trackState(element) {
    let s = trackStates.get(element);
    if (!s) {
      const kind = trackKind(element);
      s = { readyState: TRACK_NONE, url: '', track: null, media: null };
      s.track = new TextTrack(KEY, { kind, label: attr(element, 'label') || '', language: attr(element, 'srclang') || '', element });
      trackStates.set(element, s);
    }
    return s;
  }
  function trackKind(element) {
    const kind = (attr(element, 'kind') || '').toLowerCase();
    if (['subtitles', 'captions', 'descriptions', 'chapters', 'metadata'].includes(kind)) return kind;
    return attr(element, 'kind') === null ? 'subtitles' : 'metadata';
  }
  function isTrackElement(value) {
    try { return localNameOf.call(value) === 'track'; } catch (e) { return false; }
  }
  const TrackProto = global.HTMLTrackElement.prototype;
  idl(TrackProto, {
    get readyState() { if (!isTrackElement(this)) throw new TypeError('Illegal invocation'); return trackState(this).readyState; },
    get track() { if (!isTrackElement(this)) throw new TypeError('Illegal invocation'); return trackState(this).track; },
  });
  /** Fetch and parse a track element's WebVTT once its track is enabled ("start the track processing model"). */
  function loadTrack(element) {
    const s = trackState(element);
    const src = attr(element, 'src');
    if (!src) { failTrack(element, s); return; }
    let url;
    try { url = new global.URL(src, baseURI()).href; } catch (e) { failTrack(element, s); return; }
    if (s.url === url && s.readyState !== TRACK_NONE) return;
    s.url = url;
    s.readyState = TRACK_LOADING;
    const media = s.media;
    const cors = media ? attr(media, 'crossorigin') : null;
    const credentials = cors === 'use-credentials' ? 'include' : 'same-origin';
    Promise.resolve()
      .then(() => nativeFetch.call(global, url, { credentials, mode: cors === null ? 'same-origin' : 'cors' }))
      .then(response => { if (!response.ok) throw new Error('track fetch failed'); return response.text(); })
      .then(body => {
        if (s.url !== url) return;
        const list = text(s.track).cues;
        list.length = 0;
        for (const c of parseWebVTT(body)) s.track.addCue(c);
        s.readyState = TRACK_LOADED;
        queueFire(element, 'load');
      })
      .catch(() => { if (s.url === url) failTrack(element, s); });
  }
  function failTrack(element, s) {
    s.readyState = TRACK_ERROR;
    queueFire(element, 'error');
  }

  // ---- Element state. ----
  const elementStates = new WeakMap();
  const playerElements = new Map();
  let nextPlayer = 1;
  function isMediaElement(value) {
    try { const tag = localNameOf.call(value); return tag === 'video' || tag === 'audio'; } catch (e) { return false; }
  }
  function attr(element, name) { return getAttribute.call(element, name); }
  function has(element, name) { return hasAttribute.call(element, name); }
  function state(element) {
    if (!isMediaElement(element)) throw new TypeError('Illegal invocation');
    let s = elementStates.get(element);
    if (!s) {
      s = {
        element, networkState: NETWORK_EMPTY, readyState: HAVE_NOTHING, paused: true, seeking: false,
        currentSrc: '', error: null, player: 0, duration: NaN, time: 0, anchor: null, defaultStart: 0,
        defaultPlaybackRate: 1, playbackRate: 1, volume: 1, muted: has(element, 'muted'), preservesPitch: true,
        buffered: [], seekable: [], played: [], playStart: null, autoplaying: true, pending: [],
        videoWidth: 0, videoHeight: 0, loadedData: false, serial: 0, mode: null, candidates: null,
        lastProgress: 0, lastTimeupdate: 0, ended: false, srcObject: null, mediaSource: null,
        textTracks: new TextTrackList(KEY), audioTracks: new AudioTrackList(KEY), videoTracks: new VideoTrackList(KEY),
        activeCues: [], frames: 0, dropped: 0, frameCallbacks: new Map(), nextFrameCallback: 1, seekSerial: 0,
        sent: '', captions: '[]',
      };
      elementStates.set(element, s);
    }
    return s;
  }
  function video(element) { return localNameOf.call(element) === 'video'; }
  /**
   * An IDL entry point: setting src runs the load algorithm synchronously in the specification,
   * so changes the host recorded since the last stable state are applied before any member runs.
   */
  let syncing = false;
  function entry(element) {
    if (!syncing) {
      syncing = true;
      try {
        for (const changed of host.takeTouched()) {
          try { touched(changed); } catch (e) { report(e); }
        }
      } finally {
        syncing = false;
      }
    }
    return state(element);
  }

  /** The settings the host applies whole; posted whenever any of them changes. */
  function settings(s) {
    const element = s.element;
    return {
      paused: s.paused, rate: s.playbackRate, volume: s.volume, muted: s.muted,
      looping: has(element, 'loop'), preserves_pitch: s.preservesPitch, controls: has(element, 'controls'),
      preload: preload(element), video: video(element),
    };
  }
  /** The effective preload hint. An explicit load() overrides `none`, as Chromium and WebKit
   * do: pages call load() and wait for canplay before playing (Apple's inline media). */
  function preload(element) {
    const value = (attr(element, 'preload') || '').toLowerCase();
    if (value === 'none' && elementStates.get(element) && elementStates.get(element).explicitLoad) return 'metadata';
    if (value === 'none' || value === 'metadata') return value;
    return 'auto';
  }
  function post(s, command, data) {
    host.post(s.element, JSON.stringify(command), data);
  }
  function syncSettings(s) {
    if (!s.player) return;
    const value = settings(s);
    const text = JSON.stringify(value);
    if (text === s.sent) return;
    s.sent = text;
    post(s, { op: 'settings', player: s.player, settings: value });
  }

  /** The current playback position: the host's anchor advanced by the clock while playing. */
  function position(s) {
    if (s.seeking) return s.time;
    const a = s.anchor;
    if (!a || !a.playing || s.paused) return s.time;
    const advanced = a.time + (now() - a.clock) / 1000 * a.rate;
    return Number.isFinite(s.duration) ? Math.min(s.duration, advanced) : advanced;
  }
  function ended(s) {
    return s.readyState >= HAVE_METADATA && s.ended && !has(s.element, 'loop');
  }

  // ---- The load algorithm and resource selection (HTML 4.8.11.5). ----
  function load(element) {
    const s = entry(element);
    s.serial++;
    s.candidates = null;
    rejectPending(s, 'AbortError', 'The play() request was interrupted by a new load request.');
    if (s.networkState === NETWORK_LOADING || s.networkState === NETWORK_IDLE) queueFire(element, 'abort');
    if (s.networkState !== NETWORK_EMPTY) {
      queueFire(element, 'emptied');
      stopFetching(s);
      forgetTracks(s);
      s.readyState = HAVE_NOTHING;
      s.paused = true;
      s.seeking = false;
      const moved = position(s) !== 0;
      s.time = 0;
      s.anchor = null;
      s.ended = false;
      if (moved) queueFire(element, 'timeupdate');
      s.duration = NaN;
      s.buffered = [];
      s.seekable = [];
      s.played = [];
      s.videoWidth = s.videoHeight = 0;
      s.loadedData = false;
    }
    s.playbackRate = s.defaultPlaybackRate;
    s.error = null;
    s.autoplaying = true;
    selectResource(s);
  }
  function stopFetching(s) {
    if (s.player) {
      post(s, { op: 'unload', player: s.player });
      playerElements.delete(s.player);
      s.player = 0;
      s.sent = '';
    }
    if (s.mediaSource) detachSource(s.mediaSource);
  }
  function forgetTracks(s) {
    for (const list of [s.audioTracks, s.videoTracks]) {
      for (const t of listItems(list)) { track(t).list = null; queueFire(list, 'removetrack', { track: t }); }
      setItems(list, []);
    }
  }
  function selectResource(s) {
    const element = s.element;
    s.networkState = NETWORK_NO_SOURCE;
    const serial = s.serial;
    queueMicrotask.call(global, () => {
      if (s.serial !== serial) return;
      if (s.srcObject) s.mode = 'object';
      else if (has(element, 'src')) s.mode = 'attribute';
      else if (sourceChildren(element).length) s.mode = 'children';
      else { s.networkState = NETWORK_EMPTY; return; }
      s.networkState = NETWORK_LOADING;
      queueFire(element, 'loadstart');
      if (s.mode === 'object') return loadObject(s);
      if (s.mode === 'attribute') {
        const src = attr(element, 'src');
        const url = src === '' ? null : resolve(src);
        if (url === null) return queueTask(() => failDedicated(s, 'MEDIA_ELEMENT_ERROR: Empty src attribute'));
        s.currentSrc = url;
        return fetchResource(s, url);
      }
      s.candidates = sourceChildren(element);
      nextCandidate(s);
    });
  }
  function sourceChildren(element) {
    return Array.prototype.filter.call(childrenOf.call(element), node => {
      try { return localNameOf.call(node) === 'source'; } catch (e) { return false; }
    });
  }
  function resolve(src) {
    try { return new global.URL(src, baseURI()).href; } catch (e) { return null; }
  }
  /** Children mode: try each source element in order, skipping types and media that cannot play. */
  function nextCandidate(s) {
    while (s.candidates && s.candidates.length) {
      const source = s.candidates.shift();
      const src = attr(source, 'src');
      const url = src ? resolve(src) : null;
      const type = attr(source, 'type');
      const media = attr(source, 'media');
      const unplayable = type !== null && canPlay(type) === '';
      const unmatched = media && matchMedia && !matchMedia.call(global, media).matches;
      if (url === null || unplayable || unmatched) { queueFire(source, 'error'); continue; }
      s.currentSrc = url;
      s.source = source;
      return fetchResource(s, url);
    }
    s.candidates = null;
    s.networkState = NETWORK_NO_SOURCE;
  }
  function fetchResource(s, url) {
    if (s.mediaSource || mediaSources.has(url)) return attachSource(s, mediaSources.get(url));
    startPlayer(s, url, false);
  }
  function startPlayer(s, url, stream) {
    if (s.player) stopFetching(s);
    s.player = nextPlayer++;
    playerElements.set(s.player, s.element);
    const value = settings(s);
    s.sent = JSON.stringify(value);
    post(s, { op: 'load', player: s.player, url, stream, settings: value });
    if (!stream && value.preload === 'none' && !has(s.element, 'autoplay')) {
      s.networkState = NETWORK_IDLE;
      queueFire(s.element, 'suspend');
    }
  }
  function loadObject(s) {
    const object = s.srcObject;
    if (object && mediaSourceOf.has(object)) return attachSource(s, object);
    queueTask(() => failDedicated(s, 'MEDIA_ELEMENT_ERROR: Media streams are not supported'));
  }
  /** The host could not use the current resource: try the next source or fail the element. */
  function resourceFailed(s, code, message) {
    if (s.readyState === HAVE_NOTHING && s.mode === 'children') {
      if (s.source) queueFire(s.source, 'error');
      if (s.player) stopFetching(s);
      return nextCandidate(s);
    }
    if (s.readyState === HAVE_NOTHING) return failDedicated(s, message);
    stopFetching(s);
    s.error = new MediaError(KEY, code, message);
    s.networkState = NETWORK_IDLE;
    fire(s.element, 'error');
  }
  /** The "dedicated media source failure steps". */
  function failDedicated(s, message) {
    s.error = new MediaError(KEY, MEDIA_ERR_SRC_NOT_SUPPORTED, message || 'MEDIA_ELEMENT_ERROR: Format error');
    forgetTracks(s);
    if (s.player) stopFetching(s);
    s.networkState = NETWORK_NO_SOURCE;
    fire(s.element, 'error');
    rejectPending(s, 'NotSupportedError', 'Failed to load because no supported source was found.', true);
  }

  // ---- Ready state transitions (HTML "ready states"). ----
  function setReadyState(s, next) {
    const element = s.element;
    const old = s.readyState;
    if (old === next) return;
    s.readyState = next;
    if (old === HAVE_NOTHING && next >= HAVE_METADATA) {
      queueFire(element, 'loadedmetadata');
    }
    if (next >= HAVE_CURRENT_DATA && !s.loadedData) {
      s.loadedData = true;
      queueFire(element, 'loadeddata');
    }
    if (old >= HAVE_FUTURE_DATA && next <= HAVE_CURRENT_DATA) {
      if (!s.paused && !ended(s)) { queueFire(element, 'timeupdate'); queueFire(element, 'waiting'); }
      return;
    }
    if (old <= HAVE_CURRENT_DATA && next >= HAVE_FUTURE_DATA) {
      queueFire(element, 'canplay');
      if (!s.paused) notifyPlaying(s);
    }
    if (next === HAVE_ENOUGH_DATA) {
      if (eligibleForAutoplay(s)) {
        s.paused = false;
        s.autoplaying = false;
        syncSettings(s);
        queueFire(element, 'play');
        notifyPlaying(s);
      }
      queueFire(element, 'canplaythrough');
    }
  }
  function eligibleForAutoplay(s) {
    return s.autoplaying && s.paused && has(s.element, 'autoplay') && allowedToPlay(s);
  }
  /** Chrome's policy: muted playback always, audible playback after the person interacted. */
  function allowedToPlay(s) {
    return s.muted || s.volume === 0 || host.activation();
  }
  function notifyPlaying(s) {
    queueTask(() => {
      fire(s.element, 'playing');
      resolvePending(s);
    });
  }
  function resolvePending(s) {
    const pending = s.pending.splice(0, s.pending.length);
    for (const p of pending) p.resolve(undefined);
  }
  function rejectPending(s, name, message, now) {
    const pending = s.pending.splice(0, s.pending.length);
    if (!pending.length) return;
    const reject = () => { for (const p of pending) p.reject(domError(name, message)); };
    if (now) reject(); else queueTask(reject);
  }

  // ---- Playing, pausing and seeking. ----
  function play(element) {
    const s = entry(element);
    if (!allowedToPlay(s)) {
      return Promise.reject(domError('NotAllowedError', "play() failed because the user didn't interact with the document first. https://goo.gl/xX8pDD"));
    }
    if (s.error && s.error.code === MEDIA_ERR_SRC_NOT_SUPPORTED) {
      return Promise.reject(domError('NotSupportedError', 'The element has no supported sources.'));
    }
    let resolveIt, rejectIt;
    const promise = new Promise((resolve, reject) => { resolveIt = resolve; rejectIt = reject; });
    s.pending.push({ resolve: resolveIt, reject: rejectIt });
    internalPlay(s);
    return promise;
  }
  function internalPlay(s) {
    const element = s.element;
    if (s.networkState === NETWORK_EMPTY) selectResource(s);
    if (ended(s)) seek(s, 0, false);
    if (s.paused) {
      s.paused = false;
      s.ended = false;
      queueFire(element, 'play');
      if (s.readyState <= HAVE_CURRENT_DATA) queueFire(element, 'waiting');
      else notifyPlaying(s);
    } else if (s.readyState >= HAVE_FUTURE_DATA) {
      queueTask(() => resolvePending(s));
    }
    s.autoplaying = false;
    anchor(s, position(s), s.anchor ? s.anchor.playing : false);
    syncSettings(s);
  }
  function pause(element) {
    const s = entry(element);
    if (s.networkState === NETWORK_EMPTY) selectResource(s);
    internalPause(s);
  }
  function internalPause(s) {
    s.autoplaying = false;
    if (s.paused) return;
    s.time = position(s);
    s.paused = true;
    s.anchor = null;
    closePlayed(s);
    syncSettings(s);
    const element = s.element;
    const pending = s.pending.splice(0, s.pending.length);
    queueTask(() => {
      fire(element, 'timeupdate');
      fire(element, 'pause');
      for (const p of pending) p.reject(domError('AbortError', 'The play() request was interrupted by a call to pause().'));
    });
  }
  function seek(s, time, fast) {
    if (s.readyState === HAVE_NOTHING) { s.defaultStart = time; return; }
    let target = Math.max(0, time);
    if (Number.isFinite(s.duration)) target = Math.min(target, s.duration);
    closePlayed(s);
    s.seeking = true;
    s.ended = false;
    s.time = target;
    s.seekSerial++;
    post(s, { op: 'seek', player: s.player, time: target, fast: !!fast });
    queueFire(s.element, 'seeking');
  }
  function anchor(s, time, playing, rate) {
    s.time = time;
    s.anchor = { time, clock: now(), playing: playing && !s.paused, rate: rate === undefined ? s.playbackRate : rate };
  }
  /** `played` grows with each stretch of playback. */
  function closePlayed(s) {
    if (s.playStart === null) return;
    const end = position(s);
    if (end > s.playStart) s.played = normalize(s.played.concat([[s.playStart, end]]));
    s.playStart = null;
  }

  // ---- Host updates. ----
  const updateKinds = {
    metadata: metadataUpdate,
    progress: progressUpdate,
    time: timeUpdate,
    waiting: (s) => setReadyState(s, HAVE_CURRENT_DATA),
    playing: (s) => setReadyState(s, HAVE_ENOUGH_DATA),
    seeked: seekedUpdate,
    ended: endedUpdate,
    error: (s, u) => resourceFailed(s, u.code || MEDIA_ERR_SRC_NOT_SUPPORTED, u.message || ''),
    control: controlUpdate,
    frames: (s, u) => { s.frames = u.frames || 0; s.dropped = u.dropped || 0; runFrameCallbacks(s, u); },
    buffered: (s, u) => sourceBufferUpdate(s, u),
  };
  function onUpdates(json) {
    let updates;
    try { updates = JSON.parse(json); } catch (e) { return; }
    for (const u of updates) {
      const element = playerElements.get(u.player);
      if (!element || !Object.prototype.hasOwnProperty.call(updateKinds, u.kind)) continue;
      const s = state(element);
      if (s.player !== u.player) continue;
      updateKinds[u.kind](s, u);
    }
  }
  function metadataUpdate(s, u) {
    const element = s.element;
    const first = s.readyState === HAVE_NOTHING;
    if (first) {
      addMediaTracks(s, u);
      if (!Number.isNaN(u.duration) && u.duration !== s.duration) {
        s.duration = u.duration === null ? Infinity : u.duration;
        queueFire(element, 'durationchange');
      }
    } else if (u.duration !== undefined && u.duration !== s.duration && !(u.duration === null && s.duration === Infinity)) {
      s.duration = u.duration === null ? Infinity : u.duration;
      queueFire(element, 'durationchange');
    }
    if (u.seekable) s.seekable = normalize(u.seekable);
    if (video(element) && (u.width !== s.videoWidth || u.height !== s.videoHeight) && u.width && u.height) {
      s.videoWidth = u.width;
      s.videoHeight = u.height;
      queueFire(element, 'resize');
    }
    if (first) {
      setReadyState(s, HAVE_METADATA);
      const start = s.defaultStart;
      s.defaultStart = 0;
      if (start > 0) seek(s, start, false);
      setReadyState(s, u.enough ? HAVE_ENOUGH_DATA : HAVE_CURRENT_DATA);
      if (s.networkState === NETWORK_LOADING && preload(element) === 'metadata' && s.paused) {
        s.networkState = NETWORK_IDLE;
        queueFire(element, 'suspend');
      }
    }
  }
  function addMediaTracks(s, u) {
    const add = (list, cls) => {
      const t = new cls(KEY, { id: String(listItems(list).length + 1), kind: 'main', label: '', language: u.language || '' });
      track(t).list = list;
      setItems(list, listItems(list).concat([t]));
      queueFire(list, 'addtrack', { track: t });
    };
    if (u.audio) add(s.audioTracks, AudioTrack);
    if (u.video) add(s.videoTracks, VideoTrack);
  }
  function progressUpdate(s, u) {
    if (u.buffered) s.buffered = normalize(u.buffered);
    if (u.seekable) s.seekable = normalize(u.seekable);
    if (s.networkState !== NETWORK_LOADING && !u.idle) s.networkState = NETWORK_LOADING;
    const t = now();
    if (t - s.lastProgress >= PROGRESS_MS || u.idle) {
      s.lastProgress = t;
      queueFire(s.element, 'progress');
    }
    if (u.idle && s.networkState === NETWORK_LOADING) {
      s.networkState = NETWORK_IDLE;
      queueFire(s.element, 'suspend');
    }
    if (u.enough && s.readyState >= HAVE_METADATA && s.readyState < HAVE_ENOUGH_DATA) setReadyState(s, HAVE_ENOUGH_DATA);
  }
  function timeUpdate(s, u) {
    if (s.seeking || s.readyState === HAVE_NOTHING) return;
    const playing = !!u.playing && !s.paused;
    if (playing && s.playStart === null) s.playStart = u.time;
    anchor(s, u.time, playing, u.rate);
    if (playing && s.readyState >= HAVE_METADATA && s.readyState < HAVE_FUTURE_DATA) setReadyState(s, HAVE_ENOUGH_DATA);
    queueFire(s.element, 'timeupdate');
    timeMarchesOn(s.element);
  }
  function seekedUpdate(s, u) {
    if (!s.seeking || u.serial !== undefined && u.serial < s.seekSerial) return;
    s.seeking = false;
    anchor(s, u.time, !s.paused);
    if (!s.paused) s.playStart = u.time;
    timeMarchesOn(s.element);
    queueFire(s.element, 'timeupdate');
    queueFire(s.element, 'seeked');
    if (s.readyState < HAVE_FUTURE_DATA && s.readyState >= HAVE_METADATA) setReadyState(s, HAVE_ENOUGH_DATA);
  }
  function endedUpdate(s, u) {
    const element = s.element;
    s.time = Number.isFinite(s.duration) ? s.duration : u.time;
    s.anchor = null;
    closePlayed(s);
    if (has(element, 'loop')) { seek(s, 0, false); return; }
    s.ended = true;
    timeMarchesOn(element);
    queueTask(() => {
      fire(element, 'timeupdate');
      if (!s.paused && ended(s)) {
        s.paused = true;
        syncSettings(s);
        fire(element, 'pause');
      }
      fire(element, 'ended');
    });
  }
  /** A click on the native controls: toggle playback, seek on the scrubber, or toggle mute. */
  function controlUpdate(s, u) {
    if (u.action === 'mute') { setMuted(s, !s.muted); return; }
    if (u.action === 'seek' && Number.isFinite(s.duration)) { seek(s, u.time, false); return; }
    if (s.paused || ended(s)) {
      if (s.error && s.error.code === MEDIA_ERR_SRC_NOT_SUPPORTED) return;
      internalPlay(s);
    } else {
      internalPause(s);
    }
  }

  // ---- Volume, rate and the remaining attributes. ----
  function setMuted(s, value) {
    if (s.muted === value) return;
    s.muted = value;
    syncSettings(s);
    queueFire(s.element, 'volumechange');
    if (!value && !s.paused && !allowedToPlay(s)) internalPause(s);
  }

  // ---- "Time marches on": cue enter/exit, cuechange and captions. ----
  function timeMarchesOn(element) {
    const s = state(element);
    const time = position(s);
    const tracks = listItems(s.textTracks).filter(t => text(t).mode !== 'disabled');
    const current = [];
    for (const t of tracks) for (const c of text(t).cues) {
      const value = cueOf.get(c);
      if (value.startTime <= time && time < value.endTime) current.push(c);
    }
    const previous = s.activeCues;
    const exited = previous.filter(c => !current.includes(c));
    const entered = current.filter(c => !previous.includes(c));
    s.activeCues = current;
    for (const t of tracks) text(t).active = current.filter(c => cueOf.get(c).track === t);
    if (exited.some(c => cueOf.get(c).pauseOnExit) && !s.paused && !s.seeking) internalPause(s);
    const changed = new Set();
    for (const c of exited.concat(entered)) changed.add(cueOf.get(c).track);
    for (const c of exited) queueFire(c, 'exit');
    for (const c of entered) queueFire(c, 'enter');
    for (const t of changed) if (t) queueFire(t, 'cuechange');
    sendCaptions(s, tracks);
  }
  function sendCaptions(s, tracks) {
    if (!s.player) return;
    const lines = [];
    for (const t of tracks) {
      const value = text(t);
      if (value.mode !== 'showing' || !['subtitles', 'captions'].includes(value.kind)) continue;
      for (const c of value.active) if (cueOf.get(c).text !== undefined) lines.push(cueText(cueOf.get(c).text));
    }
    const json = JSON.stringify(lines);
    if (json === s.captions) return;
    s.captions = json;
    post(s, { op: 'captions', player: s.player, lines });
  }
  /** A track element joined or left a media element: keep the element's text track list in step. */
  function refreshTextTracks(media) {
    const s = state(media);
    const elements = Array.prototype.filter.call(childrenOf.call(media), isTrackElement);
    const own = listItems(s.textTracks);
    const fromElements = own.filter(t => text(t).element);
    for (const t of fromElements) {
      if (!elements.includes(text(t).element)) {
        text(t).media = null;
        setItems(s.textTracks, listItems(s.textTracks).filter(x => x !== t));
        queueFire(s.textTracks, 'removetrack', { track: t });
      }
    }
    for (const element of elements) {
      const ts = trackState(element);
      if (listItems(s.textTracks).includes(ts.track)) continue;
      ts.media = media;
      text(ts.track).media = media;
      text(ts.track).kind = trackKind(element);
      setItems(s.textTracks, listItems(s.textTracks).concat([ts.track]));
      queueFire(s.textTracks, 'addtrack', { track: ts.track });
    }
    honorDefaults(s);
  }
  /** "Honor user preferences for automatic text track selection": the default subtitles show. */
  function honorDefaults(s) {
    const tracks = listItems(s.textTracks).filter(t => text(t).element);
    const showing = tracks.some(t => text(t).mode === 'showing' && ['subtitles', 'captions'].includes(text(t).kind));
    for (const t of tracks) {
      const value = text(t);
      if (value.mode !== 'disabled' || value.defaulted || !has(value.element, 'default')) continue;
      value.defaulted = true;
      if (['subtitles', 'captions'].includes(value.kind)) {
        if (showing) continue;
        t.mode = 'showing';
      } else if (value.kind === 'chapters' || value.kind === 'metadata') {
        t.mode = 'hidden';
      }
    }
  }

  // ---- requestVideoFrameCallback over presented frames. ----
  function runFrameCallbacks(s, u) {
    if (!s.frameCallbacks.size) return;
    const callbacks = Array.from(s.frameCallbacks.values());
    s.frameCallbacks.clear();
    const t = now();
    const metadata = {
      presentationTime: t, expectedDisplayTime: t, width: s.videoWidth, height: s.videoHeight,
      mediaTime: u.time !== undefined ? u.time : position(s), presentedFrames: s.frames, processingDuration: 0,
    };
    for (const callback of callbacks) {
      try { callback.call(undefined, t, metadata); } catch (e) { report(e); }
    }
  }

  // ---- HTMLMediaElement members. ----
  const MediaProto = global.HTMLMediaElement.prototype;
  idl(MediaProto, {
    get error() { return entry(this).error; },
    get currentSrc() { return entry(this).currentSrc; },
    get networkState() { return entry(this).networkState; },
    get readyState() { return entry(this).readyState; },
    get seeking() { return entry(this).seeking; },
    get currentTime() { return position(entry(this)); },
    set currentTime(value) {
      const s = entry(this);
      const time = finite(value, 'HTMLMediaElement', 'currentTime');
      seek(s, time, false);
    },
    fastSeek(time) {
      const s = entry(this);
      seek(s, finite(time, 'HTMLMediaElement', 'fastSeek'), true);
    },
    get duration() { return entry(this).duration; },
    get paused() { return entry(this).paused; },
    get ended() { return ended(entry(this)); },
    get defaultPlaybackRate() { return entry(this).defaultPlaybackRate; },
    set defaultPlaybackRate(value) {
      const s = entry(this);
      const rate = finite(value, 'HTMLMediaElement', 'defaultPlaybackRate');
      if (rate === s.defaultPlaybackRate) return;
      s.defaultPlaybackRate = rate;
      queueFire(this, 'ratechange');
    },
    get playbackRate() { return entry(this).playbackRate; },
    set playbackRate(value) {
      const s = entry(this);
      const rate = finite(value, 'HTMLMediaElement', 'playbackRate');
      if (rate !== 0 && (Math.abs(rate) < MIN_RATE || Math.abs(rate) > MAX_RATE || rate < 0)) {
        throw domError('NotSupportedError', `Failed to set the 'playbackRate' property on 'HTMLMediaElement': The provided playback rate (${rate}) is not in the supported playback range.`);
      }
      if (rate === s.playbackRate) return;
      s.time = position(s);
      if (s.anchor) s.anchor = { time: s.time, clock: now(), playing: s.anchor.playing, rate };
      s.playbackRate = rate;
      syncSettings(s);
      queueFire(this, 'ratechange');
    },
    get preservesPitch() { return entry(this).preservesPitch; },
    set preservesPitch(value) { const s = entry(this); s.preservesPitch = !!value; syncSettings(s); },
    get played() { const s = entry(this); const extra = s.playStart !== null && !s.paused ? [[s.playStart, position(s)]] : []; return timeRanges(s.played.concat(extra)); },
    get seekable() {
      const s = entry(this);
      if (s.readyState === HAVE_NOTHING) return timeRanges([]);
      if (s.seekable.length) return timeRanges(s.seekable);
      return timeRanges(Number.isFinite(s.duration) ? [[0, s.duration]] : []);
    },
    get buffered() { return timeRanges(entry(this).buffered); },
    get volume() { return entry(this).volume; },
    set volume(value) {
      const s = entry(this);
      const volume = finite(value, 'HTMLMediaElement', 'volume');
      if (volume < 0 || volume > 1) {
        throw domError('IndexSizeError', `Failed to set the 'volume' property on 'HTMLMediaElement': The volume provided (${volume}) is outside the range [0, 1].`);
      }
      if (volume === s.volume) return;
      s.volume = volume;
      syncSettings(s);
      queueFire(this, 'volumechange');
    },
    get muted() { return entry(this).muted; },
    set muted(value) { setMuted(entry(this), !!value); },
    get srcObject() { return entry(this).srcObject; },
    set srcObject(value) {
      const s = entry(this);
      s.srcObject = value === undefined ? null : value;
      load(this);
    },
    get textTracks() { return entry(this).textTracks; },
    get audioTracks() { return entry(this).audioTracks; },
    get videoTracks() { return entry(this).videoTracks; },
    load() { entry(this).explicitLoad = true; load(this); },
    play() { return play(this); },
    pause() { pause(this); },
    canPlayType(type) { entry(this); return canPlay(String(type)); },
    addTextTrack(kind, label, language) {
      const s = entry(this);
      kind = String(kind);
      if (!['subtitles', 'captions', 'descriptions', 'chapters', 'metadata'].includes(kind)) {
        throw new TypeError(`Failed to execute 'addTextTrack' on 'HTMLMediaElement': The provided value '${kind}' is not a valid enum value of type TextTrackKind.`);
      }
      const t = new TextTrack(KEY, { kind, label: label === undefined ? '' : String(label), language: language === undefined ? '' : String(language), mode: 'hidden', media: this });
      setItems(s.textTracks, listItems(s.textTracks).concat([t]));
      queueFire(s.textTracks, 'addtrack', { track: t });
      return t;
    },
  });

  const VideoProto = global.HTMLVideoElement.prototype;
  idl(VideoProto, {
    get videoWidth() { return entry(this).videoWidth; },
    get videoHeight() { return entry(this).videoHeight; },
    getVideoPlaybackQuality() {
      const s = entry(this);
      return new VideoPlaybackQuality(KEY, { creationTime: now(), totalVideoFrames: s.frames, droppedVideoFrames: s.dropped, corruptedVideoFrames: 0 });
    },
    requestVideoFrameCallback(callback) {
      const s = entry(this);
      if (typeof callback !== 'function') throw new TypeError("Failed to execute 'requestVideoFrameCallback' on 'HTMLVideoElement': The callback provided as parameter 1 is not a function.");
      const handle = s.nextFrameCallback++;
      s.frameCallbacks.set(handle, callback);
      return handle;
    },
    cancelVideoFrameCallback(handle) { entry(this).frameCallbacks.delete(Number(handle)); },
    get webkitDecodedFrameCount() { return entry(this).frames; },
    get webkitDroppedFrameCount() { return entry(this).dropped; },
  });

  const qualityOf = new WeakMap();
  class VideoPlaybackQuality {
    constructor(token, values) {
      if (token !== KEY) illegal();
      qualityOf.set(this, values);
    }
  }
  for (const name of ['creationTime', 'totalVideoFrames', 'droppedVideoFrames', 'corruptedVideoFrames']) {
    Object.defineProperty(VideoPlaybackQuality.prototype, name, {
      get() { const v = qualityOf.get(this); if (!v) throw new TypeError('Illegal invocation'); return v[name]; },
      enumerable: true, configurable: true,
    });
  }
  expose('VideoPlaybackQuality', VideoPlaybackQuality);

  // ---- canPlayType and media capabilities, from the host's decoders. ----
  /** Parse `type/subtype; codecs="a, b"` into its essence and codec list. */
  function parseType(type) {
    const parts = String(type).split(';');
    const essence = parts[0].trim().toLowerCase();
    let codecs = null;
    for (const part of parts.slice(1)) {
      const m = /^\s*codecs\s*=\s*"?([^"]*)"?\s*$/i.exec(part);
      if (m) codecs = m[1].split(',').map(c => c.trim()).filter(Boolean);
    }
    return { essence, codecs };
  }
  function codecFamily(codec) {
    const family = codec.split('.')[0].toLowerCase();
    return family === 'mp4v' ? 'mp4v' : family;
  }
  function codecsPlayable(essence, codecs) {
    return codecs.every(c => support.codecs.includes(essence + '|' + codecFamily(c)));
  }
  function canPlay(type) {
    const { essence, codecs } = parseType(type);
    if (!support.containers.includes(essence)) return '';
    if (codecs === null) return 'maybe';
    return codecs.length && codecsPlayable(essence, codecs) ? 'probably' : '';
  }
  function streamPlayable(type) {
    const { essence, codecs } = parseType(type);
    if (!support.streams.includes(essence)) return false;
    return codecs === null || codecsPlayable(essence, codecs);
  }

  if (global.navigator && typeof global.MediaCapabilities === 'function') {
    const answer = (config, decoding) => {
      if (!config || typeof config !== 'object' || !config.type) {
        return Promise.reject(new TypeError(`Failed to execute '${decoding ? 'decodingInfo' : 'encodingInfo'}' on 'MediaCapabilities': The provided value is not a valid configuration.`));
      }
      const types = [config.video && config.video.contentType, config.audio && config.audio.contentType].filter(Boolean);
      if (!types.length) {
        return Promise.reject(new TypeError("Failed to execute 'decodingInfo' on 'MediaCapabilities': The configuration dictionary has neither |video| nor |audio| specified."));
      }
      const playable = decoding && types.every(t => config.type === 'media-source' ? streamPlayable(t) : canPlay(t) !== '');
      return Promise.resolve({
        supported: playable, smooth: playable, powerEfficient: playable && types.some(t => /avc1|hvc1|hev1|mp4a/.test(t)),
        keySystemAccess: null, configuration: config,
      });
    };
    idl(global.MediaCapabilities.prototype, {
      decodingInfo(config) { return answer(config, true); },
      encodingInfo(config) { return answer(config, false); },
    });
  }

  // ---- Media Source Extensions. ----
  const mediaSourceOf = new WeakMap();
  const mediaSources = new Map();
  let nextBuffer = 1;
  class MediaSource extends global.EventTarget {
    constructor() {
      super();
      mediaSourceOf.set(this, {
        readyState: 'closed', duration: NaN, buffers: [], active: [], element: null, live: null,
        list: new SourceBufferList(KEY, () => mediaSourceOf.get(this).buffers),
        activeList: new SourceBufferList(KEY, () => mediaSourceOf.get(this).active),
      });
    }
    static isTypeSupported(type) { return streamPlayable(type); }
    static get canConstructInDedicatedWorker() { return true; }
    get sourceBuffers() { return source(this).list; }
    get activeSourceBuffers() { return source(this).activeList; }
    get readyState() { return source(this).readyState; }
    get duration() { return source(this).duration; }
    set duration(value) {
      const m = source(this);
      const duration = Number(value);
      if (Number.isNaN(duration) || duration < 0) throw new TypeError(`Failed to set the 'duration' property on 'MediaSource': The value provided (${value}) is negative or NaN.`);
      if (m.readyState !== 'open') throw domError('InvalidStateError', "Failed to set the 'duration' property on 'MediaSource': The MediaSource's readyState is not 'open'.");
      if (m.buffers.some(b => sourceBuffer(b).updating)) throw domError('InvalidStateError', "Failed to set the 'duration' property on 'MediaSource': The 'updating' attribute is true on one or more of this MediaSource's SourceBuffers.");
      setSourceDuration(this, duration);
    }
    addSourceBuffer(type) {
      const m = source(this);
      type = String(type);
      if (!type) throw new TypeError("Failed to execute 'addSourceBuffer' on 'MediaSource': The type provided is empty.");
      if (!streamPlayable(type)) throw domError('NotSupportedError', `Failed to execute 'addSourceBuffer' on 'MediaSource': The type provided ('${type}') is unsupported.`);
      if (m.readyState !== 'open') throw domError('InvalidStateError', "Failed to execute 'addSourceBuffer' on 'MediaSource': The MediaSource's readyState is not 'open'.");
      const buffer = new SourceBuffer(KEY, this, type);
      m.buffers.push(buffer);
      m.active.push(buffer);
      queueFire(m.list, 'addsourcebuffer');
      queueFire(m.activeList, 'addsourcebuffer');
      return buffer;
    }
    removeSourceBuffer(buffer) {
      const m = source(this);
      const at = m.buffers.indexOf(buffer);
      if (at < 0) throw domError('NotFoundError', "Failed to execute 'removeSourceBuffer' on 'MediaSource': The SourceBuffer provided is not contained in this MediaSource.");
      const b = sourceBuffer(buffer);
      if (b.updating) { b.updating = false; queueFire(buffer, 'abort'); queueFire(buffer, 'updateend'); }
      m.buffers.splice(at, 1);
      m.active = m.active.filter(x => x !== buffer);
      b.removed = true;
      queueFire(m.activeList, 'removesourcebuffer');
      queueFire(m.list, 'removesourcebuffer');
    }
    endOfStream(error) {
      const m = source(this);
      if (m.readyState !== 'open') throw domError('InvalidStateError', "Failed to execute 'endOfStream' on 'MediaSource': The MediaSource's readyState is not 'open'.");
      if (m.buffers.some(b => sourceBuffer(b).updating)) throw domError('InvalidStateError', "Failed to execute 'endOfStream' on 'MediaSource': The 'updating' attribute is true on one or more of this MediaSource's SourceBuffers.");
      m.readyState = 'ended';
      queueFire(this, 'sourceended');
      const s = m.element && state(m.element);
      if (!s) return;
      if (error === 'network' || error === 'decode') {
        resourceFailed(s, error === 'network' ? MEDIA_ERR_NETWORK : MEDIA_ERR_DECODE, 'MEDIA_ELEMENT_ERROR: Media Source ended with an error');
        return;
      }
      const end = Math.max(0, ...m.buffers.map(b => { const r = sourceBuffer(b).ranges; return r.length ? r[r.length - 1][1] : 0; }));
      if (end > 0 && end !== m.duration) setSourceDuration(this, end);
      post(s, { op: 'end_of_stream', player: s.player, duration: Number.isFinite(m.duration) ? m.duration : 0 });
    }
    setLiveSeekableRange(start, end) {
      const m = source(this);
      if (m.readyState !== 'open') throw domError('InvalidStateError', "Failed to execute 'setLiveSeekableRange' on 'MediaSource': The MediaSource's readyState is not 'open'.");
      m.live = [Number(start), Number(end)];
    }
    clearLiveSeekableRange() {
      const m = source(this);
      if (m.readyState !== 'open') throw domError('InvalidStateError', "Failed to execute 'clearLiveSeekableRange' on 'MediaSource': The MediaSource's readyState is not 'open'.");
      m.live = null;
    }
  }
  handlers(MediaSource.prototype, ['sourceopen', 'sourceended', 'sourceclose']);
  function source(self) {
    const value = mediaSourceOf.get(self);
    if (!value) throw new TypeError('Illegal invocation');
    return value;
  }
  function setSourceDuration(ms, duration) {
    const m = source(ms);
    if (m.duration === duration) return;
    m.duration = duration;
    const s = m.element && state(m.element);
    if (!s) return;
    s.duration = duration;
    if (s.readyState >= HAVE_METADATA) queueFire(s.element, 'durationchange');
  }
  /** A media element selected this source: it opens and the host starts a stream player. */
  function attachSource(s, ms) {
    if (!ms) return failDedicated(s, 'MEDIA_ELEMENT_ERROR: Media Source was revoked');
    const m = source(ms);
    if (m.readyState !== 'closed') return failDedicated(s, 'MEDIA_ELEMENT_ERROR: Media Source is already attached');
    m.element = s.element;
    s.mediaSource = ms;
    startPlayer(s, s.currentSrc || 'mediasource:', true);
    m.readyState = 'open';
    queueFire(ms, 'sourceopen');
  }
  function detachSource(ms) {
    const m = source(ms);
    const element = m.element;
    m.readyState = 'closed';
    m.duration = NaN;
    m.buffers = [];
    m.active = [];
    m.element = null;
    if (element) state(element).mediaSource = null;
    queueFire(ms, 'sourceclose');
  }

  const bufferOf = new WeakMap();
  class SourceBuffer extends global.EventTarget {
    constructor(token, ms, type) {
      if (token !== KEY) illegal();
      super();
      bufferOf.set(this, {
        source: ms, type, id: nextBuffer++, mode: 'segments', updating: false, removed: false,
        ranges: [], timestampOffset: 0, appendWindowStart: 0, appendWindowEnd: Infinity, init: null, tracks: [], pending: null,
      });
    }
    get mode() { return sourceBuffer(this).mode; }
    set mode(value) {
      const b = usable(this, 'mode');
      if (value !== 'segments' && value !== 'sequence') return;
      b.mode = value;
    }
    get updating() { return sourceBuffer(this).updating; }
    get buffered() {
      const b = sourceBuffer(this);
      if (b.removed) throw domError('InvalidStateError', "Failed to read the 'buffered' property from 'SourceBuffer': This SourceBuffer has been removed from the parent media source.");
      return timeRanges(b.ranges);
    }
    get timestampOffset() { return sourceBuffer(this).timestampOffset; }
    set timestampOffset(value) { usable(this, 'timestampOffset').timestampOffset = Number(value); }
    get appendWindowStart() { return sourceBuffer(this).appendWindowStart; }
    set appendWindowStart(value) { usable(this, 'appendWindowStart').appendWindowStart = Number(value); }
    get appendWindowEnd() { return sourceBuffer(this).appendWindowEnd; }
    set appendWindowEnd(value) { usable(this, 'appendWindowEnd').appendWindowEnd = Number(value); }
    get audioTracks() { return new AudioTrackList(KEY); }
    get videoTracks() { return new VideoTrackList(KEY); }
    get textTracks() { return new TextTrackList(KEY); }
    appendBuffer(data) {
      const b = usable(this, 'appendBuffer');
      const bytes = bytesOf(data);
      if (!bytes) throw new TypeError("Failed to execute 'appendBuffer' on 'SourceBuffer': The provided value is not of type '(ArrayBuffer or ArrayBufferView)'.");
      const m = source(b.source);
      if (m.readyState === 'ended') { m.readyState = 'open'; queueFire(b.source, 'sourceopen'); }
      b.updating = true;
      queueFire(this, 'updatestart');
      const s = m.element && state(m.element);
      if (s && s.player) post(s, { op: 'append', player: s.player, buffer: b.id, mime: b.type }, bytes);
      queueTask(() => { if (b.updating) finishAppend(this, b, processAppend(b, bytes)); });
    }
    abort() {
      const b = sourceBuffer(this);
      if (b.removed) throw domError('InvalidStateError', "Failed to execute 'abort' on 'SourceBuffer': This SourceBuffer has been removed from the parent media source.");
      if (source(b.source).readyState !== 'open') throw domError('InvalidStateError', "Failed to execute 'abort' on 'SourceBuffer': The MediaSource's readyState is not 'open'.");
      if (b.updating) { b.updating = false; queueFire(this, 'abort'); queueFire(this, 'updateend'); }
      b.pending = null;
    }
    remove(start, end) {
      const b = usable(this, 'remove');
      start = Number(start); end = Number(end);
      if (Number.isNaN(start) || start < 0 || !(end > start)) throw new TypeError("Failed to execute 'remove' on 'SourceBuffer': The start provided is outside the range.");
      b.updating = true;
      queueFire(this, 'updatestart');
      const m = source(b.source);
      const s = m.element && state(m.element);
      if (s && s.player) post(s, { op: 'remove', player: s.player, buffer: b.id, start, end });
      queueTask(() => {
        b.ranges = subtract(b.ranges, start, end);
        b.updating = false;
        refreshSourceBuffered(b.source);
        fire(this, 'update');
        fire(this, 'updateend');
      });
    }
    changeType(type) {
      const b = usable(this, 'changeType');
      if (!streamPlayable(String(type))) throw domError('NotSupportedError', `Failed to execute 'changeType' on 'SourceBuffer': The type provided ('${type}') is not supported.`);
      b.type = String(type);
    }
  }
  handlers(SourceBuffer.prototype, ['updatestart', 'update', 'updateend', 'error', 'abort']);
  function sourceBuffer(self) {
    const value = bufferOf.get(self);
    if (!value) throw new TypeError('Illegal invocation');
    return value;
  }
  function usable(self, member) {
    const b = sourceBuffer(self);
    if (b.removed) throw domError('InvalidStateError', `Failed to execute '${member}' on 'SourceBuffer': This SourceBuffer has been removed from the parent media source.`);
    if (b.updating) throw domError('InvalidStateError', `Failed to execute '${member}' on 'SourceBuffer': This SourceBuffer is still processing an 'appendBuffer' or 'remove' operation.`);
    return b;
  }
  function bytesOf(data) {
    if (data instanceof ArrayBuffer) return new Uint8Array(data);
    if (ArrayBuffer.isView(data)) return new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
    return null;
  }
  function finishAppend(self, b, segment) {
    b.updating = false;
    if (segment.error) {
      b.pending = null;
      fire(self, 'error');
      fire(self, 'updateend');
      const element = source(b.source).element;
      if (element) resourceFailed(state(element), MEDIA_ERR_DECODE, 'CHUNK_DEMUXER_ERROR_APPEND_FAILED');
      return;
    }
    b.ranges = segment.ranges.map(r => [r[0], r[1]]);
    refreshSourceBuffered(b.source);
    fire(self, 'update');
    fire(self, 'updateend');
  }
  /** The element's buffered ranges are the intersection of its active source buffers'. */
  function refreshSourceBuffered(ms) {
    const m = source(ms);
    const element = m.element;
    if (!element) return;
    const s = state(element);
    const lists = m.active.map(b => sourceBuffer(b).ranges);
    s.buffered = lists.length ? lists.reduce(intersect) : [];
    const tracks = m.buffers.flatMap(b => sourceBuffer(b).tracks);
    if (s.readyState === HAVE_NOTHING && m.buffers.length && m.buffers.every(b => sourceBuffer(b).init)) {
      const v = tracks.find(t => t.kind === 'video');
      metadataUpdate(s, {
        duration: Number.isNaN(m.duration) ? null : m.duration, width: v ? v.width : 0, height: v ? v.height : 0,
        audio: tracks.some(t => t.kind === 'audio'), video: !!v, enough: false,
      });
    }
    if (s.readyState >= HAVE_METADATA) {
      const t = position(s);
      const covered = s.buffered.some(r => r[0] <= t + 0.1 && t < r[1]);
      setReadyState(s, covered ? HAVE_ENOUGH_DATA : HAVE_METADATA);
    }
  }
  function sourceBufferUpdate(s, u) {
    if (!s.mediaSource) return;
    const buffer = source(s.mediaSource).buffers.find(b => sourceBuffer(b).id === u.buffer);
    if (buffer && u.ranges) sourceBuffer(buffer).ranges = normalize(u.ranges);
    refreshSourceBuffered(s.mediaSource);
  }
  function intersect(a, b) {
    const out = [];
    for (const x of a) for (const y of b) {
      const start = Math.max(x[0], y[0]), end = Math.min(x[1], y[1]);
      if (end > start) out.push([start, end]);
    }
    return normalize(out);
  }
  function subtract(list, start, end) {
    const out = [];
    for (const r of list) {
      if (r[1] <= start || r[0] >= end) { out.push(r); continue; }
      if (r[0] < start) out.push([r[0], start]);
      if (r[1] > end) out.push([end, r[1]]);
    }
    return out;
  }

  /**
   * Coded frame processing for one append runs in the host (mars-mp4 reads the boxes and the samples); this
   * script keeps the SourceBuffer state it needs: the init segment bytes, the tracks it declared, the bytes of
   * a box still arriving, and the ranges. A parse refusal comes back as `{ error }`.
   */
  function processAppend(b, bytes) {
    const data = b.pending ? joinBytes(b.pending, bytes) : bytes;
    b.pending = null;
    const request = JSON.stringify({
      offset: b.timestampOffset, windowStart: b.appendWindowStart,
      windowEnd: Number.isFinite(b.appendWindowEnd) ? b.appendWindowEnd : null, ranges: b.ranges,
    });
    const result = JSON.parse(host.append(data, b.init, request));
    if (result.error) return { error: result.error };
    if (result.initLen) b.init = data.slice(0, result.initLen);
    if (result.tracks.length) {
      if (!tracksMatchType(b.type, result.tracks)) return { error: 'the initialization segment does not match the type' };
      b.tracks = result.tracks;
    }
    if (result.consumed < data.length) b.pending = data.slice(result.consumed);
    return { error: null, ranges: result.ranges };
  }
  function joinBytes(first, second) {
    const joined = new Uint8Array(first.length + second.length);
    joined.set(first, 0);
    joined.set(second, first.length);
    return joined;
  }
  /** Every track the init segment declares must be a codec the type names. */
  function tracksMatchType(type, tracks) {
    const { codecs } = parseType(type);
    if (codecs === null) return true;
    return tracks.every(t => codecs.some(c => codecFamily(c) === codecFamily(t.codec)));
  }
  const listSource = new WeakMap();
  class SourceBufferList extends global.EventTarget {
    constructor(token, getter) {
      if (token !== KEY) illegal();
      super();
      listSource.set(this, getter);
      return indexed(this, getter, listSource, {
        get(target, key) {
          if (typeof key === 'string' && /^(0|[1-9][0-9]*)$/.test(key)) return getter()[Number(key)];
          return Reflect.get(target, key, target);
        },
      });
    }
    get length() { const g = listSource.get(this); if (!g) throw new TypeError('Illegal invocation'); return g().length; }
    [Symbol.iterator]() { return listSource.get(this)().slice()[Symbol.iterator](); }
  }
  handlers(SourceBufferList.prototype, ['addsourcebuffer', 'removesourcebuffer']);
  expose('MediaSource', MediaSource);
  expose('SourceBuffer', SourceBuffer);
  expose('SourceBufferList', SourceBufferList);

  /** `URL.createObjectURL(mediaSource)` names the source; the element attaches when it loads it. */
  const urlMembers = {
    createObjectURL(object) {
      if (mediaSourceOf.has(object)) {
        const origin = global.location.origin === 'null' ? 'null' : global.location.origin;
        const id = randomUUID ? randomUUID() : Math.random().toString(16).slice(2);
        const url = `blob:${origin}/${id}`;
        mediaSources.set(url, object);
        return url;
      }
      return createObjectURL.call(global.URL, object);
    },
    revokeObjectURL(url) {
      if (mediaSources.delete(String(url))) return;
      return revokeObjectURL.call(global.URL, url);
    },
  };
  for (const name of Object.keys(urlMembers)) {
    const original = global.URL[name];
    const replacement = urlMembers[name];
    Object.defineProperty(replacement, 'name', { value: name });
    Object.defineProperty(replacement, 'length', { value: original.length });
    define(global.URL, name, replacement);
  }

  // ---- Element changes and host updates arrive through one dispatcher. ----
  /** An element whose src, sources or tree position changed, handled at a stable state. */
  function touched(element) {
    let tag;
    try { tag = localNameOf.call(element); } catch (e) { return; }
    if (tag === 'video' || tag === 'audio') return mediaTouched(element);
    const parent = parentOf.call(element);
    if (tag === 'source' && parent && isMediaElement(parent)) {
      const s = state(parent);
      if (s.networkState === NETWORK_EMPTY && !has(parent, 'src')) selectResource(s);
      else if (s.mode === 'children' && s.candidates === null && s.networkState === NETWORK_NO_SOURCE) {
        s.candidates = [element];
        s.networkState = NETWORK_LOADING;
        nextCandidate(s);
      }
    }
    if (tag === 'track') {
      const ts = trackState(element);
      if (ts.media && ts.media !== parent) refreshTextTracks(ts.media);
      if (parent && isMediaElement(parent)) refreshTextTracks(parent);
      if (ts.track && text(ts.track).mode !== 'disabled') { ts.url = ''; loadTrack(element); }
    }
  }
  function mediaTouched(element) {
    const s = state(element);
    const src = attr(element, 'src');
    if (s.lastSrc !== src && (src !== null || s.mode === 'attribute')) {
      s.lastSrc = src;
      if (src !== null) return load(element);
    }
    if (s.networkState === NETWORK_EMPTY && sourceChildren(element).length) selectResource(s);
    if (!connectedOf.call(element) && !s.paused) internalPause(s);
    refreshTextTracks(element);
  }
  // Elements the parser already created: their src attributes were set before this ran.
  for (const element of global.document.querySelectorAll('video, audio')) {
    try { mediaTouched(element); } catch (e) { report(e); }
  }
  return { touched, onUpdates, report };
  }

  host.listen((kind, payload) => {
    const media = ensure();
    if (kind === 'touched') {
      for (const element of payload) {
        try { media.touched(element); } catch (e) { media.report(e); }
      }
    } else if (kind === 'updates') {
      media.onUpdates(payload);
    }
  });
  // A page that has media elements from its markup builds the implementation now; any other
  // page builds it the first time it touches a media element or interface.
  if (global.document.querySelector('video, audio')) ensure();
});
