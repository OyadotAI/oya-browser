/**
 * Unit tests for src/main/tabs/session-tabs.ts: the open tabs are written once
 * the strip settles and at once on flush, never as an empty list; they come
 * back once, for the same profile, into a window showing only the start page.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { SessionTabs } from '../../../../src/main/tabs/session-tabs.ts';
import { SESSION_SAVE_DELAY_MS, RESTORED_TABS_MAX } from '../../../../src/main/tabs/constants.ts';
import { memoryFile } from '../../support/main-ctx.cjs';
import { incognitoSession } from '../../support/incognito.ts';

/** A tab on the strip. */
const tab = (id: number, url: string, extra = {}) => ({ id, url, view: {}, ...extra });

/** A session store over a fake strip, profile and window. */
function setup(saved: unknown = null) {
  const file = memoryFile(saved);
  const list: any[] = [tab(1, 'oya:home', { home: true })];
  const calls: unknown[][] = [];
  let next = 10;
  const deps: any = {
    tabs: {
      list,
      activeTabId: 1,
      createTab: (url: string, activate: boolean) => (
        calls.push(['open', url, activate]),
        list.push(tab(next, url)),
        next++
      ),
      activateTab: (id: number) => calls.push(['activate', id]),
      find: (id: number) => list.find((t) => t.id === id),
      sendTabList: () => calls.push(['send']),
      closeTab: (id: number) => (
        calls.push(['close', id]),
        list.splice(
          list.findIndex((t) => t.id === id),
          1,
        )
      ),
    },
    persona: { partitionName: () => 'persist:one' },
    shell: { browsingMode: true },
    governance: { configuration: null },
  };
  return { file, list, calls, deps, session: new SessionTabs(deps, file) };
}

describe('SessionTabs', () => {
  beforeEach(() => mock.timers.enable({ apis: ['setTimeout'] }));
  afterEach(() => mock.timers.reset());

  it('saves the open tabs and the active one once the strip settles', () => {
    const { file, list, deps, session } = setup();
    list.push(tab(2, 'https://a.test/'), tab(3, 'https://b.test/'));
    deps.tabs.activeTabId = 3;
    session.changed();
    session.changed();
    assert.equal(file.read(), null, 'not yet');
    mock.timers.tick(SESSION_SAVE_DELAY_MS);
    assert.deepEqual(file.read(), {
      partition: 'persist:one',
      urls: ['https://a.test/', 'https://b.test/'],
      active: 1,
      pinned: 0,
    });
  });

  it('never saves an incognito tab, even one that reached the strip', async () => {
    const { file, list, session } = setup();
    session.restore();
    const view = { webContents: { session: await incognitoSession() } };
    list.push(tab(2, 'https://private.test/', { view }), tab(3, 'https://a.test/'));
    session.flush();
    assert.deepEqual(file.read(), { partition: 'persist:one', urls: ['https://a.test/'], active: 0, pinned: 0 });
  });

  it('leaves out the start page, popups, private and blank tabs, and writes an empty session for a start page alone', () => {
    const { file, list, session } = setup();
    session.restore();
    list.push(tab(2, 'about:blank'), tab(3, 'https://pop.test/', { window: {} }), tab(4, 'file:///etc/hosts'));
    session.flush();
    assert.deepEqual(file.read(), { partition: 'persist:one', urls: [], active: 0, pinned: 0 });
  });

  it('keeps saved tabs while startup waits for cookies, including quitting before reconnect', () => {
    const saved = { partition: 'persist:one', urls: ['https://a.test/'], active: 0 };
    const { file, calls, session } = setup(saved);
    session.changed();
    mock.timers.tick(SESSION_SAVE_DELAY_MS);
    session.flush();
    assert.deepEqual(file.read(), saved);
    session.restore();
    assert.deepEqual(calls[0], ['open', 'https://a.test/', false]);
  });

  it('saves an empty session when the person closes every real page after startup', () => {
    const { file, list, session } = setup();
    list.push(tab(2, 'https://a.test/'));
    session.flush();
    list.pop();
    session.flush();
    assert.deepEqual(file.read(), { partition: 'persist:one', urls: [], active: 0, pinned: 0 });
  });

  it('never writes an empty strip: that is a bulk close, not a session', () => {
    const { file, list, session } = setup({ partition: 'persist:one', urls: ['https://a.test/'], active: 0 });
    list.splice(0);
    session.changed();
    mock.timers.tick(SESSION_SAVE_DELAY_MS);
    session.flush();
    assert.deepEqual(file.read().urls, ['https://a.test/']);
  });

  it('flush writes at once and cancels the pending save', () => {
    const { file, list, session } = setup();
    list.push(tab(2, 'https://a.test/'));
    session.changed();
    session.flush();
    assert.deepEqual(file.read().urls, ['https://a.test/']);
    list.push(tab(3, 'https://b.test/'));
    mock.timers.tick(SESSION_SAVE_DELAY_MS);
    assert.deepEqual(file.read().urls, ['https://a.test/'], 'the pending save was cancelled');
  });

  it('restores once, only into a window showing just the start page, with the saved tab active, and closes that start page', () => {
    const saved = { partition: 'persist:one', urls: ['https://a.test/', 'https://b.test/'], active: 1 };
    const { calls, session } = setup(saved);
    session.restore();
    session.restore();
    assert.deepEqual(calls, [
      ['open', 'https://a.test/', false],
      ['open', 'https://b.test/', false],
      ['activate', 11],
      ['close', 1],
    ]);
  });

  it('restores nothing for another profile, a governed browser, before browsing mode, a page already open, or a corrupt file', () => {
    const saved = { partition: 'persist:one', urls: ['https://a.test/'], active: 0 };
    for (const [name, change, file] of [
      ['profile', (d) => (d.persona.partitionName = () => 'persist:two'), saved],
      ['governed', (d) => (d.governance.configuration = {}), saved],
      ['setup', (d) => (d.shell.browsingMode = false), saved],
      ['open page', (d) => d.tabs.list.push(tab(2, 'https://typed.test/')), saved],
      ['corrupt', () => {}, 'garbage'],
      ['no urls', () => {}, { partition: 'persist:one' }],
    ] as any[]) {
      const { calls, deps, session } = setup(file);
      change(deps);
      session.restore();
      assert.deepEqual(calls, [], name);
    }
  });

  it('restores at most RESTORED_TABS_MAX, in order, and only web addresses: never file:, javascript: or non-strings', () => {
    const urls = Array.from({ length: RESTORED_TABS_MAX + 5 }, (_v, i) => `https://a.test/${i}`);
    const { calls, session } = setup({
      partition: 'persist:one',
      urls: [42, 'file:///etc/passwd', 'javascript:alert(1)', ...urls],
      active: 99,
    });
    session.restore();
    const opened = calls.filter(([kind]) => kind === 'open');
    assert.equal(opened.length, RESTORED_TABS_MAX);
    assert.equal(opened[0][1], 'https://a.test/0');
    assert.deepEqual(
      calls.find(([kind]) => kind === 'activate'),
      ['activate', 10],
      'an out-of-range active falls back to the first',
    );
  });
});

describe('SessionTabs pinned tabs', () => {
  beforeEach(() => mock.timers.enable({ apis: ['setTimeout'] }));
  afterEach(() => mock.timers.reset());

  it('counts only the pinned tabs the file keeps: a pinned start page is not one', () => {
    const { file, list, session } = setup();
    list[0].pinned = true;
    list.push(tab(2, 'https://mail.test/', { pinned: true }), tab(3, 'https://x.test/'));
    session.flush();
    assert.deepEqual(file.read().urls, ['https://mail.test/', 'https://x.test/']);
    assert.equal(file.read().pinned, 1);
  });

  it('saves pinned tabs first whatever the window order, and indexes active into that order', () => {
    const { file, list, deps, session } = setup();
    list.push(tab(2, 'https://a.test/'), tab(3, 'https://b.test/', { pinned: true }));
    deps.tabs.activeTabId = 2;
    session.flush();
    assert.deepEqual(file.read(), {
      partition: 'persist:one',
      urls: ['https://b.test/', 'https://a.test/'],
      active: 1,
      pinned: 1,
    });
  });

  it('restores the first N tabs pinned, flagged after the start page closes, and redraws the strip', () => {
    const saved = {
      partition: 'persist:one',
      urls: ['https://a.test/', 'https://b.test/', 'https://c.test/'],
      active: 0,
      pinned: 2,
    };
    const { list, calls, session } = setup(saved);
    session.restore();
    assert.deepEqual(
      list.map((t) => [t.url, !!t.pinned]),
      [
        ['https://a.test/', true],
        ['https://b.test/', true],
        ['https://c.test/', false],
      ],
    );
    assert.deepEqual(calls.slice(-2), [['close', 1], ['send']]);
  });

  it('restores no pins from a file written before pinning', () => {
    const { list, calls, session } = setup({ partition: 'persist:one', urls: ['https://a.test/'], active: 0 });
    session.restore();
    assert.equal(list[0].pinned, undefined);
    assert.equal(
      calls.some(([kind]) => kind === 'send'),
      false,
    );
  });

  it('clamps a pinned count past the addresses, and ignores a negative or non-integer one', () => {
    for (const [pinned, expected] of [
      [9, 2],
      [-1, 0],
      [1.5, 0],
      ['2', 0],
    ] as const) {
      const { list, session } = setup({
        partition: 'persist:one',
        urls: ['https://a.test/', 'https://b.test/'],
        active: 0,
        pinned,
      });
      session.restore();
      assert.equal(list.filter((t) => t.pinned).length, expected, String(pinned));
    }
  });
});
