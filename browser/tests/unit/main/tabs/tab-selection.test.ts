/** Return selection survives strip reordering and externally removed popup tabs. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { TabSelection } from '../../../../src/main/tabs/tab-selection.ts';
it('returns the last live selected tab, not the last strip item', () => {
  const selection = new TabSelection();
  selection.visit(1);
  selection.visit(3);
  selection.visit(2);
  selection.forget(2);
  assert.equal(selection.previous([{ id: 3 }, { id: 1 }]), 3);
  assert.equal(selection.previous([{ id: 1 }]), 1);
  assert.equal(selection.previous([]), undefined);
});
it('cycles in both directions and handles an empty tab list', () => {
  const selection = new TabSelection();
  assert.equal(selection.cycle([{ id: 1 }, { id: 2 }], 1, -1), 2);
  assert.equal(selection.cycle([{ id: 1 }, { id: 2 }], 2, 1), 1);
  assert.equal(selection.cycle([], null, 1), undefined);
});
