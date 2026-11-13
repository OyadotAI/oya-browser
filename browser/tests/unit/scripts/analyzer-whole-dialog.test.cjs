/** A modal read covers the whole dialog, so its footer buttons (Save, Cancel) stay reachable. */
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

const source = fs.readFileSync(path.join(__dirname, '../../../scripts/analyzer.js'), 'utf8');

/** The analyzer's own text from `start` to the end of that declaration. */
function slice(start, end) {
  const from = source.indexOf(start);
  assert.ok(from >= 0, `analyzer.js declares ${start.trim()}`);
  return source.slice(from, source.indexOf(end, from) + end.length);
}

const selectorDecl = slice('  const DIALOG_BOX_SELECTOR', ';\n');
const wholeDialogDecl = slice('  function wholeDialog(', '\n  }\n');

/** An element that is a dialog box or not, with the parent chain closest() walks. */
function node(isBox, parent = null) {
  const n = {
    parentElement: parent,
    matches: (selector) => {
      assert.equal(selector, context.DIALOG_BOX_SELECTOR);
      return isBox;
    },
    closest: (selector) => {
      for (let p = n; p; p = p.parentElement) if (p.matches(selector)) return p;
      return null;
    },
  };
  return n;
}

const context = {};
vm.runInNewContext(
  `${selectorDecl}\n${wholeDialogDecl}\nthis.DIALOG_BOX_SELECTOR = DIALOG_BOX_SELECTOR; this.wholeDialog = wholeDialog;`,
  context,
);

test('a dialog body widens to the dialog around it, so the footer is read', () => {
  const dialog = node(true, node(false));
  const body = node(false, dialog);
  assert.equal(context.wholeDialog(body), dialog);
});

test('a match that is already a dialog is kept, even inside another dialog', () => {
  const outer = node(true);
  const inner = node(true, outer);
  assert.equal(context.wholeDialog(inner), inner);
});

test('a body with no dialog around it is read on its own', () => {
  const body = node(false, node(false));
  assert.equal(context.wholeDialog(body), body);
});

test('the dialog box selector names LinkedIn modals and standard dialogs', () => {
  for (const part of ['[role="dialog"]', '[role="alertdialog"]', 'dialog', '[aria-modal="true"]', '.artdeco-modal'])
    assert.ok(context.DIALOG_BOX_SELECTOR.includes(part), part);
  assert.ok(!context.DIALOG_BOX_SELECTOR.includes('.artdeco-modal__content'));
});

/** The analyzer's ariaName, given a document that holds the elements labelledby points at. */
function ariaNameIn(texts) {
  const ctx = { document: { getElementById: (id) => (id in texts ? { textContent: texts[id] } : null) } };
  vm.runInNewContext(`${slice('  function ariaName(', '\n  }\n')}\nthis.ariaName = ariaName;`, ctx);
  return (attrs) => ctx.ariaName({ getAttribute: (name) => attrs[name] ?? null, ownerDocument: null });
}

test('a dialog named by aria-labelledby reads as the heading text, never the id', () => {
  const name = ariaNameIn({ 'ember123-header': '  Comment, react, and\n repost as ' });
  assert.equal(name({ 'aria-labelledby': 'ember123-header' }), 'Comment, react, and repost as');
});

test('aria-label wins over aria-labelledby', () => {
  const name = ariaNameIn({ h: 'Heading' });
  assert.equal(name({ 'aria-label': 'Share', 'aria-labelledby': 'h' }), 'Share');
});

test('a labelledby id with no element names nothing', () => {
  assert.equal(ariaNameIn({})({ 'aria-labelledby': 'ember123-header' }), '');
});
