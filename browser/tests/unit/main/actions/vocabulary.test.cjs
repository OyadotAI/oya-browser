/**
 * Unit tests for the actions this app announces: exactly what its command
 * maps do, sorted and without the server's internal ones, so the server's
 * check and the browser detail never disagree with the app.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const Module = require('node:module');
const { OYA_ACTIONS } = require('../../../../main/actions/vocabulary.cjs');

/** Loads a main-process module with Electron stubbed, as these maps require it at load time. */
function withoutElectron(load) {
  const original = Module._load;
  const stub = new Proxy(function () {}, { get: () => stub, apply: () => stub });
  Module._load = function (request, ...rest) {
    return request === 'electron' ? stub : original.call(this, request, ...rest);
  };
  try {
    return load();
  } finally {
    Module._load = original;
  }
}

/** Actions only the server sends: done by the app, never announced. */
const INTERNAL = new Set(['evaluate_raw', 'record']);

describe('OYA_ACTIONS', () => {
  it('equals the actions the command maps answer, plus handle_dialog, without the internal ones', () => {
    const maps = withoutElectron(() => [
      require('../../../../main/actions/page-commands.cjs').PAGE_COMMANDS,
      require('../../../../main/connection/tab-commands.cjs').TAB_COMMANDS,
      require('../../../../main/actions/scripts.cjs').ACTION_SCRIPTS,
    ]);
    const answered = new Set(['handle_dialog', ...maps.flatMap((map) => Object.keys(map))]);
    assert.deepEqual(OYA_ACTIONS, [...answered].filter((a) => !INTERNAL.has(a)).sort());
  });
});
