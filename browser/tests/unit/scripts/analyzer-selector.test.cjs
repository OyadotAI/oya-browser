/** Public analyzer references must target their numeric value, never digits in randomized attribute names. */
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const source = fs.readFileSync(path.join(__dirname, '../../../scripts/analyzer.js'), 'utf8');
const start = source.indexOf('  window.__acFindElement = function (selector) {');
const implementation = source.slice(start, source.indexOf('\n  };', start) + '\n  };'.length);
/** Run the actual published lookup function with live DOM reference seams. */
function fixture() {
  const references = new Map([3, 6, 9, 34].map((id) => [id, { id, isConnected: true }]));
  const context = {
    window: {},
    elementRefs: references,
    ATTR: 'data-f9c3c8b3',
    elementMap: [],
    document: { querySelector: () => null },
    queryShadow: () => null,
    findReplacementElement: () => null,
  };
  vm.runInNewContext(implementation, context);
  return { find: context.window.__acFindElement, references };
}
for (const reference of [
  3,
  '3',
  '[data-f9c3c8b3="3"]',
  "[data-f9c3c8b3='3']",
  '[data-ac-id="3"]',
  ' [data-123456="3"] ',
])
  test(`lookup resolves the value of ${reference}`, () => {
    const { find, references } = fixture();
    assert.equal(find(reference), references.get(3));
  });
test('Amazon-style random attribute digits cannot select another numbered control', () => {
  const { find, references } = fixture();
  assert.equal(find('[data-cbda3f34="6"]'), references.get(6));
});
for (const reference of [
  'button3',
  '#search3',
  '[data-f9c3c8b3="3\']',
  '[data-f9c3c8b3="3"] button',
  '3junk',
  -3,
  0,
  3.5,
  Infinity,
  '9007199254740993',
  '[data-ac-id="0"]',
])
  test(`malformed or non-reference selector is refused: ${reference}`, () => {
    assert.equal(fixture().find(reference), null);
  });
