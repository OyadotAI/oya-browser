/** Wrapped inline targets use a reachable line box, and native clicks use that same point. */
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const source = fs.readFileSync(path.join(__dirname, '../../../scripts/analyzer.js'), 'utf8');
const start = source.indexOf('  function isCovered(dom)');
const end = source.indexOf('  /** Whether an element sits', start);
/** Exact injected functions run against geometry/hit-test seams, without a browser substitute. */
function fixture(
  hit,
  rects = [
    { left: 70, top: 10, right: 100, bottom: 30 },
    { left: 0, top: 30, right: 25, bottom: 50 },
  ],
) {
  const win = { innerWidth: 100, innerHeight: 100, frameElement: null };
  const root = { elementFromPoint: (x, y) => hit(x, y, el) };
  const el = {
    getClientRects: () => rects,
    ownerDocument: { defaultView: win },
    getRootNode: () => root,
    contains: (node) => node === el.child,
  };
  el.child = {};
  const context = { window: {}, document: el.ownerDocument, semanticsOnly: false };
  vm.runInNewContext(source.slice(start, end), context);
  return { point: () => context.window.__acReachablePoint(el), el, win, root, context };
}
test('an uncovered second line is reachable when the union centre and first line are blocked', () => {
  const f = fixture((x, y, el) => (y > 30 ? el : {}));
  assert.deepEqual(JSON.parse(JSON.stringify(f.point())), { x: 12.5, y: 40 });
});
test('all line boxes covered by an unrelated overlay remain covered', () => {
  assert.equal(fixture(() => ({})).point(), null);
});
test('a parent paragraph at the sampled point does not count as reaching a link', () => {
  const parent = { contains: () => true };
  assert.equal(fixture(() => parent).point(), null);
});
test('a descendant at a line centre reaches its enclosing interactive element', () => {
  assert.ok(fixture((x, y, el) => el.child).point());
});
test('client rectangles are clipped and empty or offscreen boxes are skipped', () => {
  const f = fixture(
    (x, y, el) => {
      assert.equal(x, 5);
      assert.equal(y, 10);
      return el;
    },
    [
      { left: 200, top: 0, right: 210, bottom: 20 },
      { left: -20, top: 0, right: 10, bottom: 20 },
    ],
  );
  assert.ok(f.point());
});
test('an outer overlay still covers an otherwise reachable framed link', () => {
  const f = fixture((x, y, el) => el);
  const outerRoot = { elementFromPoint: () => ({}) };
  const outerDocument = { defaultView: { frameElement: null } };
  f.context.document = outerDocument;
  f.win.frameElement = {
    ownerDocument: outerDocument,
    getBoundingClientRect: () => ({ left: 0, top: 0, width: 100, height: 100 }),
    clientLeft: 0,
    clientTop: 0,
    offsetWidth: 100,
    offsetHeight: 100,
    getRootNode: () => outerRoot,
    contains: () => false,
  };
  assert.equal(f.point(), null);
});
test('shadow-root traversal must also reach its host in the outer document', () => {
  const f = fixture((x, y, el) => el);
  f.root.host = { getRootNode: () => ({ elementFromPoint: () => null }), contains: () => false };
  assert.equal(f.point(), null);
});

test('a reachable point in a scaled iframe maps to the exact top-frame click coordinate', () => {
  const f = fixture((x, y, el) => el, [{ left: 0, top: 100, right: 25, bottom: 120, width: 25, height: 20 }]);
  f.win.innerHeight = 200;
  const outerDocument = { defaultView: { frameElement: null } };
  const frame = {
    ownerDocument: outerDocument,
    clientLeft: 0,
    clientTop: 0,
    offsetWidth: 100,
    offsetHeight: 200,
    getBoundingClientRect: () => ({ left: 100, top: 50, width: 200, height: 400 }),
    getRootNode: () => ({ elementFromPoint: (x, y) => (x === 125 && y === 270 ? frame : {}) }),
    contains: () => false,
  };
  f.win.frameElement = frame;
  f.context.document = outerDocument;
  assert.deepEqual(JSON.parse(JSON.stringify(f.point())), { x: 125, y: 270 });
});
