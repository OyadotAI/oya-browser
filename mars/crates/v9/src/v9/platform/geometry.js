/**
 * The Geometry Interfaces: DOMPoint(ReadOnly), DOMRect(ReadOnly), DOMRectList, DOMQuad and a
 * full 4x4 DOMMatrix(ReadOnly), as the W3C Geometry Interfaces Module specifies them. Chrome's
 * shapes made these exist with illegal constructors and inert methods, so Spotify's player
 * (`new DOMRect(...)`) and Apple's animation library (`new DOMMatrix(transform).inverse()`)
 * stopped. Element.getBoundingClientRect, getClientRects and the canvas getTransform hand out
 * these classes' instances, so `instanceof DOMRect` holds as in Chrome. State lives in a
 * WeakMap, out of the page's reach.
 */
(function installGeometry(global) {
  'use strict';
  /** Instance to its internal fields. */
  const slots = new WeakMap();
  /** The fields of a geometry object, or a TypeError for a foreign receiver. */
  function own(self) {
    const value = slots.get(self);
    if (!value) throw new TypeError('Illegal invocation');
    return value;
  }
  /** WebIDL unrestricted double conversion. */
  const num = (value, fallback) => (value === undefined ? fallback : Number(value));
  /** Radians in one degree. */
  const DEG = Math.PI / 180;
  /** Row-major indexes of the 16 components m11..m44 inside the column-major storage. */
  const NAMES = ['m11', 'm12', 'm13', 'm14', 'm21', 'm22', 'm23', 'm24', 'm31', 'm32', 'm33', 'm34', 'm41', 'm42', 'm43', 'm44'];
  /** The 2D aliases and the component each one names. */
  const ALIASES = { a: 'm11', b: 'm12', c: 'm21', d: 'm22', e: 'm41', f: 'm42' };
  /** Components that must be 0 (or 1 on the diagonal) for a matrix to stay 2D. */
  const DEPTH = ['m13', 'm14', 'm23', 'm24', 'm31', 'm32', 'm34', 'm43'];

  function define(target, name, value) {
    Object.defineProperty(target, name, { value, writable: true, enumerable: false, configurable: true });
  }
  /** Read-only accessors for each field name, reading the instance's slots. */
  function getters(proto, names) {
    for (const name of names) {
      Object.defineProperty(proto, name, { get() { return own(this)[name]; }, enumerable: true, configurable: true });
    }
  }
  /** Writable accessors over the same slots, for the mutable subclass. */
  function accessors(proto, names, after) {
    for (const name of names) {
      Object.defineProperty(proto, name, {
        get() { return own(this)[name]; },
        set(value) { own(this)[name] = Number(value); if (after) after(own(this), name); },
        enumerable: true, configurable: true,
      });
    }
  }

  /* ---------------------------------------------------------------- DOMPoint */
  class DOMPointReadOnly {
    constructor(x, y, z, w) {
      slots.set(this, { x: num(x, 0), y: num(y, 0), z: num(z, 0), w: num(w, 1) });
    }
    static fromPoint(other = {}) { return new this(other.x, other.y, other.z, other.w); }
    matrixTransform(matrix = {}) {
      const m = matrix instanceof DOMMatrixReadOnly ? own(matrix) : own(DOMMatrix.fromMatrix(matrix));
      const p = own(this);
      return transformPoint(m, p);
    }
    toJSON() { const p = own(this); return { x: p.x, y: p.y, z: p.z, w: p.w }; }
  }
  getters(DOMPointReadOnly.prototype, ['x', 'y', 'z', 'w']);
  class DOMPoint extends DOMPointReadOnly {}
  accessors(DOMPoint.prototype, ['x', 'y', 'z', 'w']);

  /* ----------------------------------------------------------------- DOMRect */
  class DOMRectReadOnly {
    constructor(x, y, width, height) {
      slots.set(this, { x: num(x, 0), y: num(y, 0), width: num(width, 0), height: num(height, 0) });
    }
    static fromRect(other = {}) { return new this(other.x, other.y, other.width, other.height); }
    get top() { const r = own(this); return Math.min(r.y, r.y + r.height); }
    get right() { const r = own(this); return Math.max(r.x, r.x + r.width); }
    get bottom() { const r = own(this); return Math.max(r.y, r.y + r.height); }
    get left() { const r = own(this); return Math.min(r.x, r.x + r.width); }
    toJSON() {
      return { x: this.x, y: this.y, width: this.width, height: this.height, top: this.top, right: this.right, bottom: this.bottom, left: this.left };
    }
  }
  getters(DOMRectReadOnly.prototype, ['x', 'y', 'width', 'height']);
  class DOMRect extends DOMRectReadOnly {}
  accessors(DOMRect.prototype, ['x', 'y', 'width', 'height']);

  class DOMRectList {
    constructor() { throw new TypeError('Illegal constructor'); }
    get length() { return own(this).rects.length; }
    item(index) { const rects = own(this).rects; const i = index >>> 0; return i < rects.length ? rects[i] : null; }
    [Symbol.iterator]() { return own(this).rects[Symbol.iterator](); }
  }
  /** A DOMRectList over `rects`, with index properties as the interface's indexed getter gives. */
  function rectList(rects) {
    const list = Object.create(DOMRectList.prototype);
    slots.set(list, { rects });
    rects.forEach((rect, index) => Object.defineProperty(list, index, { value: rect, enumerable: true, configurable: true }));
    return list;
  }

  /* ----------------------------------------------------------------- DOMQuad */
  class DOMQuad {
    constructor(p1 = {}, p2 = {}, p3 = {}, p4 = {}) {
      slots.set(this, { p1: DOMPoint.fromPoint(p1), p2: DOMPoint.fromPoint(p2), p3: DOMPoint.fromPoint(p3), p4: DOMPoint.fromPoint(p4) });
    }
    static fromRect(other = {}) {
      const x = num(other.x, 0), y = num(other.y, 0), w = num(other.width, 0), h = num(other.height, 0);
      return new DOMQuad({ x, y }, { x: x + w, y }, { x: x + w, y: y + h }, { x, y: y + h });
    }
    static fromQuad(other = {}) { return new DOMQuad(other.p1, other.p2, other.p3, other.p4); }
    getBounds() {
      const q = own(this);
      const xs = [q.p1.x, q.p2.x, q.p3.x, q.p4.x], ys = [q.p1.y, q.p2.y, q.p3.y, q.p4.y];
      const left = Math.min(...xs), top = Math.min(...ys);
      return new DOMRect(left, top, Math.max(...xs) - left, Math.max(...ys) - top);
    }
    toJSON() { const q = own(this); return { p1: q.p1, p2: q.p2, p3: q.p3, p4: q.p4 }; }
  }
  getters(DOMQuad.prototype, ['p1', 'p2', 'p3', 'p4']);

  /* --------------------------------------------------------------- DOMMatrix */
  /** A fresh identity field set: m11..m44 and the 2D flag. */
  function identity() {
    const m = { is2D: true };
    NAMES.forEach((name, i) => { m[name] = i % 5 === 0 ? 1 : 0; });
    return m;
  }
  /** Fields from 16 numbers in column-major order (m11, m12, ..., m44). */
  function from16(values, is2D) {
    const m = { is2D };
    NAMES.forEach((name, i) => { m[name] = values[i]; });
    return m;
  }
  /** Fields from the 2D six (a, b, c, d, e, f). */
  function from6([a, b, c, d, e, f]) {
    return from16([a, b, 0, 0, c, d, 0, 0, 0, 0, 1, 0, e, f, 0, 1], true);
  }
  /** The 16 components in column-major order. */
  const values = (m) => NAMES.map((name) => m[name]);

  /** `left * right` (apply right first, as post-multiplication does), as fields. */
  function multiply(left, right) {
    const l = values(left), r = values(right), out = new Array(16);
    for (let col = 0; col < 4; col++) {
      for (let row = 0; row < 4; row++) {
        let sum = 0;
        for (let k = 0; k < 4; k++) sum += l[k * 4 + row] * r[col * 4 + k];
        out[col * 4 + row] = sum;
      }
    }
    return from16(out, left.is2D && right.is2D);
  }

  /** The inverse fields, or all-NaN non-2D fields when the matrix cannot be inverted. */
  function invert(m) {
    const a = values(m), inv = new Array(16);
    inv[0] = a[5] * a[10] * a[15] - a[5] * a[11] * a[14] - a[9] * a[6] * a[15] + a[9] * a[7] * a[14] + a[13] * a[6] * a[11] - a[13] * a[7] * a[10];
    inv[4] = -a[4] * a[10] * a[15] + a[4] * a[11] * a[14] + a[8] * a[6] * a[15] - a[8] * a[7] * a[14] - a[12] * a[6] * a[11] + a[12] * a[7] * a[10];
    inv[8] = a[4] * a[9] * a[15] - a[4] * a[11] * a[13] - a[8] * a[5] * a[15] + a[8] * a[7] * a[13] + a[12] * a[5] * a[11] - a[12] * a[7] * a[9];
    inv[12] = -a[4] * a[9] * a[14] + a[4] * a[10] * a[13] + a[8] * a[5] * a[14] - a[8] * a[6] * a[13] - a[12] * a[5] * a[10] + a[12] * a[6] * a[9];
    inv[1] = -a[1] * a[10] * a[15] + a[1] * a[11] * a[14] + a[9] * a[2] * a[15] - a[9] * a[3] * a[14] - a[13] * a[2] * a[11] + a[13] * a[3] * a[10];
    inv[5] = a[0] * a[10] * a[15] - a[0] * a[11] * a[14] - a[8] * a[2] * a[15] + a[8] * a[3] * a[14] + a[12] * a[2] * a[11] - a[12] * a[3] * a[10];
    inv[9] = -a[0] * a[9] * a[15] + a[0] * a[11] * a[13] + a[8] * a[1] * a[15] - a[8] * a[3] * a[13] - a[12] * a[1] * a[11] + a[12] * a[3] * a[9];
    inv[13] = a[0] * a[9] * a[14] - a[0] * a[10] * a[13] - a[8] * a[1] * a[14] + a[8] * a[2] * a[13] + a[12] * a[1] * a[10] - a[12] * a[2] * a[9];
    inv[2] = a[1] * a[6] * a[15] - a[1] * a[7] * a[14] - a[5] * a[2] * a[15] + a[5] * a[3] * a[14] + a[13] * a[2] * a[7] - a[13] * a[3] * a[6];
    inv[6] = -a[0] * a[6] * a[15] + a[0] * a[7] * a[14] + a[4] * a[2] * a[15] - a[4] * a[3] * a[14] - a[12] * a[2] * a[7] + a[12] * a[3] * a[6];
    inv[10] = a[0] * a[5] * a[15] - a[0] * a[7] * a[13] - a[4] * a[1] * a[15] + a[4] * a[3] * a[13] + a[12] * a[1] * a[7] - a[12] * a[3] * a[5];
    inv[14] = -a[0] * a[5] * a[14] + a[0] * a[6] * a[13] + a[4] * a[1] * a[14] - a[4] * a[2] * a[13] - a[12] * a[1] * a[6] + a[12] * a[2] * a[5];
    inv[3] = -a[1] * a[6] * a[11] + a[1] * a[7] * a[10] + a[5] * a[2] * a[11] - a[5] * a[3] * a[10] - a[9] * a[2] * a[7] + a[9] * a[3] * a[6];
    inv[7] = a[0] * a[6] * a[11] - a[0] * a[7] * a[10] - a[4] * a[2] * a[11] + a[4] * a[3] * a[10] + a[8] * a[2] * a[7] - a[8] * a[3] * a[6];
    inv[11] = -a[0] * a[5] * a[11] + a[0] * a[7] * a[9] + a[4] * a[1] * a[11] - a[4] * a[3] * a[9] - a[8] * a[1] * a[7] + a[8] * a[3] * a[5];
    inv[15] = a[0] * a[5] * a[10] - a[0] * a[6] * a[9] - a[4] * a[1] * a[10] + a[4] * a[2] * a[9] + a[8] * a[1] * a[6] - a[8] * a[2] * a[5];
    const det = a[0] * inv[0] + a[1] * inv[4] + a[2] * inv[8] + a[3] * inv[12];
    if (det === 0 || !Number.isFinite(det)) return from16(new Array(16).fill(NaN), false);
    return from16(inv.map((v) => v / det), m.is2D);
  }

  /** A point mapped through the matrix, as a new DOMPoint. */
  function transformPoint(m, p) {
    const x = m.m11 * p.x + m.m21 * p.y + m.m31 * p.z + m.m41 * p.w;
    const y = m.m12 * p.x + m.m22 * p.y + m.m32 * p.z + m.m42 * p.w;
    const z = m.m13 * p.x + m.m23 * p.y + m.m33 * p.z + m.m43 * p.w;
    const w = m.m14 * p.x + m.m24 * p.y + m.m34 * p.z + m.m44 * p.w;
    return new DOMPoint(x, y, z, w);
  }

  /** Elementary transforms as fields. */
  const translation = (x, y, z) => from16([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, x, y, z, 1], z === 0);
  const scaling = (x, y, z) => from16([x, 0, 0, 0, 0, y, 0, 0, 0, 0, z, 0, 0, 0, 0, 1], z === 1);
  const skewing = (ax, ay) => from6([1, Math.tan(ay * DEG), Math.tan(ax * DEG), 1, 0, 0]);
  /** Rotation of `angle` degrees about the axis (x, y, z), as CSS rotate3d defines it. */
  function axisRotation(x, y, z, angle) {
    const length = Math.hypot(x, y, z);
    if (length === 0) return identity();
    [x, y, z] = [x / length, y / length, z / length];
    const half = (angle * DEG) / 2, sc = Math.sin(half) * Math.cos(half), sq = Math.sin(half) ** 2;
    const is2D = x === 0 && y === 0;
    return from16([
      1 - 2 * (y * y + z * z) * sq, 2 * (x * y * sq + z * sc), 2 * (x * z * sq - y * sc), 0,
      2 * (x * y * sq - z * sc), 1 - 2 * (x * x + z * z) * sq, 2 * (y * z * sq + x * sc), 0,
      2 * (x * z * sq + y * sc), 2 * (y * z * sq - x * sc), 1 - 2 * (x * x + y * y) * sq, 0,
      0, 0, 0, 1,
    ], is2D);
  }

  /** A length in px for transform strings; other units cannot be resolved without layout. */
  function length(text) {
    if (/^-?[\d.]+(e-?\d+)?(px)?$/i.test(text)) return parseFloat(text);
    throw new DOMException(`Failed to parse '${text}'.`, 'SyntaxError');
  }
  /** An angle in degrees from deg, rad, grad or turn (a bare number only when zero). */
  function angle(text) {
    const value = parseFloat(text);
    if (/deg$/i.test(text)) return value;
    if (/grad$/i.test(text)) return value * 0.9;
    if (/rad$/i.test(text)) return value / DEG;
    if (/turn$/i.test(text)) return value * 360;
    if (value === 0) return 0;
    throw new DOMException(`Failed to parse '${text}'.`, 'SyntaxError');
  }
  /** One transform function as fields. */
  function transformFunction(name, args) {
    const n = args.map(Number);
    switch (name) {
      case 'matrix': if (n.length === 6) return from6(n); break;
      case 'matrix3d': if (n.length === 16) return from16(n, false); break;
      case 'translate': return translation(length(args[0]), args[1] ? length(args[1]) : 0, 0);
      case 'translatex': return translation(length(args[0]), 0, 0);
      case 'translatey': return translation(0, length(args[0]), 0);
      case 'translatez': return translation(0, 0, length(args[0]));
      case 'translate3d': return translation(length(args[0]), length(args[1]), length(args[2]));
      case 'scale': return scaling(n[0], n.length > 1 ? n[1] : n[0], 1);
      case 'scalex': return scaling(n[0], 1, 1);
      case 'scaley': return scaling(1, n[0], 1);
      case 'scalez': return scaling(1, 1, n[0]);
      case 'scale3d': return scaling(n[0], n[1], n[2]);
      case 'rotate': case 'rotatez': return axisRotation(0, 0, 1, angle(args[0]));
      case 'rotatex': return axisRotation(1, 0, 0, angle(args[0]));
      case 'rotatey': return axisRotation(0, 1, 0, angle(args[0]));
      case 'rotate3d': return axisRotation(n[0], n[1], n[2], angle(args[3]));
      case 'skew': return skewing(angle(args[0]), args[1] ? angle(args[1]) : 0);
      case 'skewx': return skewing(angle(args[0]), 0);
      case 'skewy': return skewing(0, angle(args[0]));
      case 'perspective': {
        const d = length(args[0]);
        const m = identity(); m.is2D = false; if (d !== 0) m.m34 = -1 / d; return m;
      }
      default: break;
    }
    throw new DOMException(`Failed to parse '${name}(${args.join(', ')})'.`, 'SyntaxError');
  }
  /** A CSS transform list (or `none`/empty) as fields, composed left to right. */
  function parseTransform(text) {
    const source = String(text).trim();
    if (source === '' || source.toLowerCase() === 'none') return identity();
    let m = identity();
    const pattern = /([a-z0-9]+)\(([^)]*)\)/gi;
    let match, consumed = '';
    while ((match = pattern.exec(source))) {
      const args = match[2].split(/[\s,]+/).filter(Boolean);
      m = multiply(m, transformFunction(match[1].toLowerCase(), args));
      consumed += match[0];
    }
    if (consumed.replace(/\s/g, '') !== source.replace(/\s/g, '')) {
      throw new DOMException(`Failed to parse '${source}'.`, 'SyntaxError');
    }
    return m;
  }

  /** Fields from a DOMMatrixInit dictionary, validated and fixed up as the specification says. */
  function fromInit(init = {}) {
    if (init instanceof DOMMatrixReadOnly) return { ...own(init) };
    const m = identity();
    for (const [alias, name] of Object.entries(ALIASES)) {
      const a = init[alias], full = init[name];
      if (a !== undefined && full !== undefined && !Object.is(Number(a), Number(full))) {
        throw new TypeError(`The '${alias}' and '${name}' members do not match.`);
      }
      if (full !== undefined) m[name] = Number(full); else if (a !== undefined) m[name] = Number(a);
    }
    let flat = true;
    for (const name of DEPTH.concat(['m33', 'm44'])) {
      if (init[name] !== undefined) m[name] = Number(init[name]);
    }
    for (const name of DEPTH) if (m[name] !== 0) flat = false;
    if (m.m33 !== 1 || m.m44 !== 1) flat = false;
    if (init.is2D === true && !flat) throw new TypeError('The is2D member is true but the matrix is not 2D.');
    m.is2D = init.is2D === undefined ? flat : Boolean(init.is2D);
    return m;
  }
  /** Fields from a constructor argument: nothing, a CSS string, or a 6/16 number sequence. */
  function fromArgument(init) {
    if (init === undefined) return identity();
    if (typeof init === 'string') {
      if (!global.document) throw new TypeError("Failed to construct 'DOMMatrix': Matrix creation from a string is only supported in windows.");
      return parseTransform(init);
    }
    const list = Array.from(init, Number);
    if (list.length === 6) return from6(list);
    if (list.length === 16) return from16(list, false);
    throw new TypeError(`Failed to construct 'DOMMatrix': The sequence must contain 6 elements for a 2D matrix or 16 elements for a 3D matrix.`);
  }

  class DOMMatrixReadOnly {
    constructor(init) { slots.set(this, fromArgument(init)); }
    static fromMatrix(other) { const m = new this(); slots.set(m, fromInit(other)); return m; }
    static fromFloat32Array(array) { return new this(Array.from(array)); }
    static fromFloat64Array(array) { return new this(Array.from(array)); }
    get is2D() { return own(this).is2D; }
    get isIdentity() { const m = own(this); return NAMES.every((name, i) => m[name] === (i % 5 === 0 ? 1 : 0)); }
    translate(x, y, z) { return DOMMatrix.fromMatrix(this).translateSelf(x, y, z); }
    scale(sx, sy, sz, ox, oy, oz) { return DOMMatrix.fromMatrix(this).scaleSelf(sx, sy, sz, ox, oy, oz); }
    scaleNonUniform(sx, sy) { return DOMMatrix.fromMatrix(this).scaleSelf(sx, num(sy, 1), 1, 0, 0, 0); }
    scale3d(scale, ox, oy, oz) { return DOMMatrix.fromMatrix(this).scale3dSelf(scale, ox, oy, oz); }
    rotate(rx, ry, rz) { return DOMMatrix.fromMatrix(this).rotateSelf(rx, ry, rz); }
    rotateFromVector(x, y) { return DOMMatrix.fromMatrix(this).rotateFromVectorSelf(x, y); }
    rotateAxisAngle(x, y, z, a) { return DOMMatrix.fromMatrix(this).rotateAxisAngleSelf(x, y, z, a); }
    skewX(sx) { return DOMMatrix.fromMatrix(this).skewXSelf(sx); }
    skewY(sy) { return DOMMatrix.fromMatrix(this).skewYSelf(sy); }
    multiply(other) { return DOMMatrix.fromMatrix(this).multiplySelf(other); }
    flipX() { return DOMMatrix.fromMatrix(this).multiplySelf({ a: -1, b: 0, c: 0, d: 1, e: 0, f: 0 }); }
    flipY() { return DOMMatrix.fromMatrix(this).multiplySelf({ a: 1, b: 0, c: 0, d: -1, e: 0, f: 0 }); }
    inverse() { return DOMMatrix.fromMatrix(this).invertSelf(); }
    transformPoint(point = {}) { return transformPoint(own(this), DOMPoint.fromPoint(point)); }
    toFloat32Array() { return new Float32Array(values(own(this))); }
    toFloat64Array() { return new Float64Array(values(own(this))); }
    toJSON() {
      const m = own(this), json = { is2D: m.is2D, isIdentity: this.isIdentity };
      for (const [alias, name] of Object.entries(ALIASES)) json[alias] = m[name];
      for (const name of NAMES) json[name] = m[name];
      return json;
    }
    toString() {
      const m = own(this);
      if (!values(m).every(Number.isFinite)) throw new DOMException('Cannot stringify a matrix with non-finite values.', 'InvalidStateError');
      if (m.is2D) return `matrix(${[m.m11, m.m12, m.m21, m.m22, m.m41, m.m42].join(', ')})`;
      return `matrix3d(${values(m).join(', ')})`;
    }
  }
  getters(DOMMatrixReadOnly.prototype, NAMES);
  for (const [alias, name] of Object.entries(ALIASES)) {
    Object.defineProperty(DOMMatrixReadOnly.prototype, alias, { get() { return own(this)[name]; }, enumerable: true, configurable: true });
  }

  /** Replace the receiver's fields with `left * right`'s and return the receiver. */
  function apply(self, next) {
    const m = own(self);
    Object.assign(m, next);
    return self;
  }
  class DOMMatrix extends DOMMatrixReadOnly {
    multiplySelf(other) { return apply(this, multiply(own(this), fromInit(other))); }
    preMultiplySelf(other) { return apply(this, multiply(fromInit(other), own(this))); }
    translateSelf(x, y, z) {
      return apply(this, multiply(own(this), translation(num(x, 0), num(y, 0), num(z, 0))));
    }
    scaleSelf(sx, sy, sz, ox, oy, oz) {
      const x = num(sx, 1), y = num(sy, x), z = num(sz, 1), o = [num(ox, 0), num(oy, 0), num(oz, 0)];
      let next = multiply(own(this), translation(o[0], o[1], o[2]));
      next = multiply(next, scaling(x, y, z));
      return apply(this, multiply(next, translation(-o[0], -o[1], -o[2])));
    }
    scale3dSelf(scale, ox, oy, oz) { const s = num(scale, 1); return this.scaleSelf(s, s, s, ox, oy, oz); }
    rotateSelf(rx, ry, rz) {
      let x = num(rx, 0), y = num(ry, undefined), z = num(rz, undefined);
      if (y === undefined && z === undefined) { z = x; x = 0; y = 0; }
      y = y === undefined ? 0 : y; z = z === undefined ? 0 : z;
      let next = multiply(own(this), axisRotation(0, 0, 1, z));
      next = multiply(next, axisRotation(0, 1, 0, y));
      return apply(this, multiply(next, axisRotation(1, 0, 0, x)));
    }
    rotateFromVectorSelf(x, y) {
      const vx = num(x, 0), vy = num(y, 0);
      const degrees = vx === 0 && vy === 0 ? 0 : Math.atan2(vy, vx) / DEG;
      return apply(this, multiply(own(this), axisRotation(0, 0, 1, degrees)));
    }
    rotateAxisAngleSelf(x, y, z, a) {
      return apply(this, multiply(own(this), axisRotation(num(x, 0), num(y, 0), num(z, 0), num(a, 0))));
    }
    skewXSelf(sx) { return apply(this, multiply(own(this), skewing(num(sx, 0), 0))); }
    skewYSelf(sy) { return apply(this, multiply(own(this), skewing(0, num(sy, 0)))); }
    invertSelf() { return apply(this, invert(own(this))); }
    setMatrixValue(text) { return apply(this, parseTransform(text)); }
  }
  /** Writing a depth component makes the matrix 3D for good, as the specification says. */
  accessors(DOMMatrix.prototype, NAMES, (m, name) => { if (DEPTH.includes(name) || ((name === 'm33' || name === 'm44') && m[name] !== 1)) m.is2D = false; });
  for (const [alias, name] of Object.entries(ALIASES)) {
    Object.defineProperty(DOMMatrix.prototype, alias, {
      get() { return own(this)[name]; }, set(value) { own(this)[name] = Number(value); }, enumerable: true, configurable: true,
    });
  }

  /* ------------------------------------------------------- platform wiring */
  /** Make a native method return geometry instances built from what it returned. */
  function convert(start, name, map) {
    let owner = start;
    while (owner && !Object.prototype.hasOwnProperty.call(owner, name)) owner = Object.getPrototypeOf(owner);
    if (!owner || typeof owner[name] !== 'function') return;
    const original = owner[name];
    const wrapped = { [name](...args) { return map(original.apply(this, args)); } }[name];
    define(owner, name, wrapped);
  }
  const toRect = (r) => (r ? new DOMRect(r.x ?? r.left, r.y ?? r.top, r.width, r.height) : r);
  const toList = (list) => rectList(Array.from(list || [], toRect));
  if (global.Element) {
    convert(global.Element.prototype, 'getBoundingClientRect', toRect);
    convert(global.Element.prototype, 'getClientRects', toList);
  }
  if (global.Range) {
    convert(global.Range.prototype, 'getBoundingClientRect', toRect);
    convert(global.Range.prototype, 'getClientRects', toList);
  }
  for (const name of ['CanvasRenderingContext2D', 'OffscreenCanvasRenderingContext2D']) {
    if (global[name]) convert(global[name].prototype, 'getTransform', (m) => DOMMatrix.fromMatrix({ a: m.a, b: m.b, c: m.c, d: m.d, e: m.e, f: m.f }));
  }

  const exported = { DOMPointReadOnly, DOMPoint, DOMRectReadOnly, DOMRect, DOMRectList, DOMQuad, DOMMatrixReadOnly, DOMMatrix };
  for (const [name, value] of Object.entries(exported)) define(global, name, value);
  if (global.document) define(global, 'WebKitCSSMatrix', DOMMatrix);
})(globalThis);
