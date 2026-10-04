/**
 * Unit tests for what a tab shows (icon, title, close label, hover-card
 * address) and how a tab list merges into the strip.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import * as M from '../../../../../../src/renderer/features/tabs/model/tab-model.ts';

/** A tab as the main process summarizes it. */
const tab = (id: number, extra: Partial<M.Tab> = {}): M.Tab => ({
  id,
  title: `Tab ${id}`,
  url: `https://${id}.test/`,
  home: false,
  favicon: null,
  active: false,
  ...extra,
});

/** Items for `tabs`, at rest. */
const items = (...tabs: M.Tab[]): M.TabItemModel[] => tabs.map((t) => ({ tab: t, closing: false, opening: false }));

/** The merge options at rest. */
const REST = { holdOrder: false, animateNew: false, fold: true };

describe('tab model', () => {
  it("shows a spinner while loading, the Oya mark at home, the page's favicon, else a globe", () => {
    assert.equal(M.iconKind(tab(1, { loading: true, home: true })), 'loading');
    assert.equal(M.iconKind(tab(1, { home: true })), 'home');
    assert.deepEqual(M.iconKind(tab(1, { favicon: 'data:image/png;base64,AA' })), {
      favicon: 'data:image/png;base64,AA',
    });
    assert.equal(M.iconKind(tab(1)), 'globe');
  });

  it('names an untitled tab and its close button', () => {
    assert.equal(M.tabTitle(tab(1, { title: '' })), 'New tab');
    assert.equal(M.closeLabel(tab(1, { title: '' })), 'Close tab');
    assert.equal(M.closeLabel(tab(1)), 'Close Tab 1');
  });

  it('reads the load error by the name the main process sends, or the shared one', () => {
    assert.equal(M.loadError(tab(1, { loadError: 'refused' })), 'refused');
    assert.equal(M.loadError(tab(1, { error: 'gone' })), 'gone');
    assert.equal(M.loadError(tab(1)), null);
  });

  it("tells the card's address without the scheme, or where the tab is", () => {
    assert.equal(M.cardAddress(tab(1, { url: 'https://a.test/x?y=1' })), 'a.test/x?y=1');
    assert.equal(M.cardAddress(tab(1, { home: true })), 'Oya start page');
    assert.equal(M.cardAddress(tab(1, { url: '' })), 'Blank page');
  });

  it("follows the main process's order", () => {
    const merged = M.mergeItems(items(tab(1), tab(2), tab(3)), [tab(3), tab(1), tab(2)], REST);
    assert.deepEqual(
      merged.map((item) => item.tab.id),
      [3, 1, 2],
    );
  });

  it('holds the shown order during a press, adding new tabs at the end', () => {
    const merged = M.mergeItems(items(tab(1), tab(2)), [tab(2), tab(1), tab(3)], { ...REST, holdOrder: true });
    assert.deepEqual(
      merged.map((item) => item.tab.id),
      [1, 2, 3],
    );
  });

  it('folds a closed tab where it stood, or drops it at once without folding', () => {
    const folded = M.mergeItems(items(tab(1), tab(2), tab(3)), [tab(1), tab(3)], REST);
    assert.deepEqual(
      folded.map((item) => [item.tab.id, item.closing]),
      [
        [1, false],
        [2, true],
        [3, false],
      ],
    );
    const dropped = M.mergeItems(items(tab(1), tab(2)), [tab(1)], { ...REST, fold: false });
    assert.deepEqual(
      dropped.map((item) => item.tab.id),
      [1],
    );
  });

  it('grows in only tabs that are new, when asked', () => {
    const merged = M.mergeItems(items(tab(1)), [tab(1), tab(2)], { ...REST, animateNew: true });
    assert.deepEqual(
      merged.map((item) => item.opening),
      [false, true],
    );
  });

  it('moves a tab among the open ones, leaving folding ones in place', () => {
    const strip = [...items(tab(1), tab(2)), { tab: tab(9), closing: true, opening: false }, ...items(tab(3))];
    assert.deepEqual(
      M.moveItem(strip, 1, 2).map((item) => item.tab.id),
      [2, 3, 9, 1],
    );
    assert.deepEqual(
      M.moveItem(strip, 42, 0).map((item) => item.tab.id),
      [1, 2, 9, 3],
      'an unknown tab moves nothing',
    );
  });
});
