/**
 * Unit tests for the preload bridge: each method invokes its channel, the
 * reduced-motion preference is filled in, and each event hands over its payload
 * alone and can be unsubscribed.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { buildBridge, type RendererIpc } from '../../../src/preload/bridge.ts';

/** A fake ipcRenderer that records calls and lets a test fire events. */
function fakeIpc() {
  const invoked: unknown[][] = [];
  const listeners = new Map<string, Set<(event: unknown, payload: unknown) => void>>();
  const ipc: RendererIpc = {
    invoke: async (...args) => void invoked.push(args),
    on: (channel, fn) => listeners.set(channel, (listeners.get(channel) ?? new Set()).add(fn)),
    removeListener: (channel, fn) => listeners.get(channel)?.delete(fn),
  };
  const emit = (channel: string, payload: unknown) => listeners.get(channel)?.forEach((fn) => fn({}, payload));
  return { ipc, invoked, emit };
}

describe('preload bridge', () => {
  it('invokes the matching channel with the arguments', () => {
    const { ipc, invoked } = fakeIpc();
    const api = buildBridge(ipc, () => false);
    api.navigate('https://a.test');
    api.saveRecording('name', 'what');
    api.devAction('click', { id: 1 });
    api.moveTab(3, 0);
    api.showTabMenu(3);
    assert.deepEqual(invoked, [
      ['navigate', 'https://a.test'],
      ['save-recording', 'name', 'what'],
      ['dev-action', 'click', { id: 1 }],
      ['move-tab', 3, 0],
      ['tab-menu', 3],
    ]);
  });

  it('passes the reduced-motion preference when toggling the dev panel', () => {
    const { ipc, invoked } = fakeIpc();
    buildBridge(ipc, (query) => query.includes('reduce')).toggleDevPanel();
    assert.deepEqual(invoked, [['toggle-dev-panel', true]]);
  });

  it('hands each event payload to the listener without the IPC event, until unsubscribed', () => {
    const { ipc, emit } = fakeIpc();
    const api = buildBridge(ipc, () => false);
    const got: unknown[] = [];
    const stop = api.onTabsUpdated((tabs) => got.push(tabs));
    emit('tabs-updated', [{ id: 1 }]);
    stop();
    emit('tabs-updated', [{ id: 2 }]);
    assert.deepEqual(got, [[{ id: 1 }]]);
  });

  it('shares one IPC listener per channel among its subscribers, and lets it go after the last', () => {
    const { ipc, emit } = fakeIpc();
    let attached = 0;
    const on = ipc.on;
    ipc.on = (channel, fn) => (attached++, on(channel, fn));
    const api = buildBridge(ipc, () => false);
    const got: unknown[] = [];
    const stops = [api.onWsStatus((s) => got.push(['a', s])), api.onWsStatus((s) => got.push(['b', s]))];
    emit('ws-status', 1);
    stops.forEach((stop) => stop());
    emit('ws-status', 2);
    assert.equal(attached, 1);
    assert.deepEqual(got, [
      ['a', 1],
      ['b', 1],
    ]);
  });
});
