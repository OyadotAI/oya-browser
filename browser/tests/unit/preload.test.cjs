/**
 * Unit tests for preload.js: window.oyaBrowser maps each method to its IPC
 * channel and each listener to its event, with a faked Electron.
 */
const { describe, it, before, after } = require('node:test');
const assert = require('node:assert/strict');
const { installElectron, freshRequire } = require('./support/fakes.cjs');

describe('preload', () => {
  const invoked = [];
  const listeners = {};
  let api;
  let restore;
  before(() => {
    globalThis.window = { matchMedia: (query) => ({ matches: query.includes('reduce') }) };
    restore = installElectron({
      contextBridge: { exposeInMainWorld: (name, value) => name === 'oyaBrowser' && (api = value) },
      ipcRenderer: {
        invoke: (...args) => invoked.push(args),
        on: (channel, fn) => (listeners[channel] = fn),
      },
    });
    freshRequire('preload.js');
  });
  after(() => {
    restore();
    delete globalThis.window;
  });

  it('invokes the matching channel with the arguments', () => {
    api.navigate('https://a.test');
    api.saveRecording('name', 'what');
    api.devAction('click', { id: 1 });
    api.saveChatPlaybook('lookup');
    assert.deepEqual(invoked.slice(-4), [
      ['navigate', 'https://a.test'],
      ['save-recording', 'name', 'what'],
      ['dev-action', 'click', { id: 1 }],
      ['save-chat-playbook', 'lookup'],
    ]);
  });

  it('passes the reduced-motion preference when toggling the dev panel', () => {
    api.toggleDevPanel();
    assert.deepEqual(invoked.at(-1), ['toggle-dev-panel', true]);
  });

  it('hands each event payload to the listener without the IPC event', () => {
    const got = [];
    api.onTabsUpdated((tabs) => got.push(tabs));
    api.onControlState((state) => got.push(state));
    listeners['tabs-updated']({ sender: 'x' }, [{ id: 1 }]);
    listeners['control-state']({}, { mode: 'agent' });
    assert.deepEqual(got, [[{ id: 1 }], { mode: 'agent' }]);
  });
});
