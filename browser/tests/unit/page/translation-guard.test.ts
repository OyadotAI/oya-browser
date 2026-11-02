/** Execute the real isolated translation scripts to verify document and run fences under reordered operations. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createContext, runInContext } from 'node:vm';
import { identifyJs, collectFor, applyFor, restoreFor } from '../../../src/page/translation-guard.ts';

/** A tiny DOM seam; execution uses the production script text in its own persistent world. */
function fixture() {
  const node = { nodeType: 3, data: 'Bonjour' };
  const element = {
    nodeType: 1,
    tagName: 'BODY',
    childNodes: [node],
    getAttribute: () => null,
    classList: { contains: () => false },
  };
  Object.assign(node, { parentNode: element });
  const context = createContext({
    document: { body: element, documentElement: { ...element, getAttribute: () => 'fr' }, querySelector: () => null },
    getComputedStyle: () => ({ display: 'block' }),
    MutationObserver: class {
      /** This fixture has no asynchronous DOM mutations. */
      observe() {}
      /** There is no native observer to release. */
      disconnect() {}
    },
  });
  const run = (script: string) => runInContext(script, context);
  run(identifyJs('document-a'));
  return { node, run };
}

test('late apply after restoration cannot rewrite the original', () => {
  const { node, run } = fixture();
  run(collectFor('document-a', 1, 20000, 4000));
  run(applyFor('document-a', 1, 0, ['Hello']));
  assert.equal(node.data, 'Hello');
  run(restoreFor('document-a', 1));
  run(applyFor('document-a', 1, 0, ['Late']));
  assert.equal(node.data, 'Bonjour');
});

test('restoration before a queued collection prevents that run from restarting', () => {
  const { node, run } = fixture();
  run(restoreFor('document-a', 1));
  assert.throws(() => run(collectFor('document-a', 1, 20000, 4000)), /run ended/);
  run(applyFor('document-a', 1, 0, ['Late']));
  assert.equal(node.data, 'Bonjour');
});

test('old document operations cannot collect, overwrite or restore the current document', () => {
  const { node, run } = fixture();
  run(collectFor('document-a', 2, 20000, 4000));
  run(applyFor('document-a', 2, 0, ['Hello']));
  assert.throws(() => run(collectFor('document-old', 3, 20000, 4000)), /document changed/);
  run(applyFor('document-old', 2, 0, ['Wrong']));
  run(restoreFor('document-old', 2));
  assert.equal(node.data, 'Hello');
});

test('a delayed old-run restore preserves the newer translation', () => {
  const { node, run } = fixture();
  run(collectFor('document-a', 1, 20000, 4000));
  run(applyFor('document-a', 1, 0, ['Hello']));
  run(restoreFor('document-a', 1));
  run(collectFor('document-a', 2, 20000, 4000));
  run(applyFor('document-a', 2, 0, ['Good day']));
  run(restoreFor('document-a', 1));
  assert.equal(node.data, 'Good day');
});

test('a delayed identification cannot replace a document token already established', () => {
  const { run } = fixture();
  assert.equal(run(identifyJs('late-seed')).document, 'document-a');
});
