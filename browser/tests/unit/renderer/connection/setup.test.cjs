/**
 * Unit tests for the manual setup form (renderer/connection/setup.js): which
 * server addresses it accepts, and that a refused one is never saved.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { loadRenderer } = require('../../support/renderer-harness.cjs');

/** Why the form refuses `server` with a key filled in, or '' when it accepts it. */
const problemWith = (app, server) => app.run(`Setup.problem(${JSON.stringify(server)}, 'oya_key')`);

describe('the manual setup form', () => {
  it('accepts wss:// to any host', () => {
    const app = loadRenderer();
    assert.equal(problemWith(app, 'wss://oya.example.com/ws'), '');
  });

  it('accepts plaintext ws:// to this machine', () => {
    const app = loadRenderer();
    for (const server of ['ws://localhost:8080/ws', 'ws://127.0.0.1:8080', 'ws://[::1]:8080/ws', 'ws://LOCALHOST/ws']) {
      assert.equal(problemWith(app, server), '', server);
    }
  });

  it('refuses plaintext ws:// to any other host', () => {
    const app = loadRenderer();
    for (const server of [
      'ws://oya.example.com/ws',
      'ws://10.0.0.5:8080/ws',
      'ws://localhost.evil.test/ws',
      'ws://localhost@evil.test/ws',
      'ws://localhost:80@evil.test',
    ]) {
      assert.match(problemWith(app, server), /wss:\/\//, server);
    }
  });

  it('does not save a refused address', () => {
    const app = loadRenderer();
    app.$('cfg-server').value = 'ws://oya.example.com/ws';
    app.$('cfg-key').value = 'oya_key';
    app.fire(app.$('btn-connect'), 'click');
    assert.equal(app.bridge.called('saveConfig').length, 0);
    assert.match(app.$('setup-error').textContent, /only for this computer/);
  });
});
