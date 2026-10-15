/**
 * Unit tests for the translate channels in src/main/ipc/navigation.ts: each
 * reaches the translator only for the person's own tab in this window.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { NavigationHandlers } from '../../../../src/main/ipc/navigation.ts';

/** Handlers over one tab (id 3), with an agent holding the tabs in `held`. */
function handlers(held: number[] = []) {
  const tab = { id: 3 };
  const calls: Array<[string, unknown]> = [];
  const deps: any = {
    tabs: { find: (id: number) => (id === tab.id ? tab : undefined) },
    agentTab: {
      requireFor: (id: number) => {
        if (held.includes(id)) throw new Error('Switch to Human Mode before using this page');
      },
    },
    translation: {
      translate: async (t: unknown) => calls.push(['translate', t]),
      showOriginal: async (t: unknown) => calls.push(['original', t]),
    },
  };
  return { h: new NavigationHandlers(deps).handlers, calls, tab };
}

describe('translate channels', () => {
  it('translate-page and show-original reach the translator with the tab', async () => {
    const { h, calls, tab } = handlers();
    await h['translate-page']({} as any, 3);
    await h['show-original']({} as any, 3);
    assert.deepEqual(calls, [
      ['translate', tab],
      ['original', tab],
    ]);
  });

  it('refuse a tab an agent holds', () => {
    const { h, calls } = handlers([3]);
    assert.throws(() => h['translate-page']({} as any, 3), /Human Mode/);
    assert.throws(() => h['show-original']({} as any, 3), /Human Mode/);
    assert.deepEqual(calls, []);
  });

  it('refuse a tab that is not in this window', () => {
    const { h } = handlers();
    assert.throws(() => h['translate-page']({} as any, 99), /not in this window/);
  });
});
