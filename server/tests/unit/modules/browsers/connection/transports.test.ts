/**
 * Unit tests for the command transport strategies: a driven (CDP) browser and
 * an Oya client socket.
 */
import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { DriverTransport } from '../../../../../src/modules/browsers/connection/transports/driver-transport.ts';
import { SocketTransport } from '../../../../../src/modules/browsers/connection/transports/socket-transport.ts';
import { pendingCommands } from '../../../../../src/modules/browsers/connection/transports/pending-commands.ts';
import { describeCall } from '../../../../../src/modules/browsers/connection/reporter.ts';
import { takeDialogNote } from '../../../../../src/modules/browsers/connection/dialog-notes.ts';
import { registry } from '../../../../../src/modules/browsers/registry.ts';
import { FakeSocket, connectBrowser, disconnectBrowser } from '../../../support/fakes.ts';

const BROWSER = 'b-transport';

describe('DriverTransport', () => {
  afterEach(() => disconnectBrowser(BROWSER));

  it('returns the driver result and learns the page URL from it', async () => {
    connectBrowser(BROWSER);
    const driver = { send: async () => ({ ok: true, data: { url: 'https://example.com/next' } }) };
    const result = await new DriverTransport(driver).send(describeCall(BROWSER, 'navigate', {}, 1000));
    assert.equal(result.ok, true);
    assert.equal(registry.get(BROWSER).currentUrl, 'https://example.com/next');
  });

  it('keeps a dialog the command ran into for the next result', async () => {
    connectBrowser(BROWSER);
    const driver = { send: async () => ({ ok: true, data: { dialog: 'alert("hi") accepted' } }) };
    await new DriverTransport(driver).send(describeCall(BROWSER, 'click', {}, 1000));
    assert.equal(takeDialogNote(BROWSER), 'alert("hi") accepted');
    assert.equal(takeDialogNote(BROWSER), null, 'a note is reported once');
  });

  it('passes a driver failure through', async () => {
    const driver = { send: async () => Promise.reject(new Error('target closed')) };
    await assert.rejects(new DriverTransport(driver).send(describeCall(BROWSER, 'click', {}, 1000)), /target closed/);
  });
});

describe('SocketTransport', () => {
  afterEach(() => disconnectBrowser(BROWSER));

  it('sends a cmd message and resolves with the matching cmd_result', async () => {
    const ws = connectBrowser(BROWSER);
    const answer = new SocketTransport(ws).send(describeCall(BROWSER, 'scroll', { direction: 'down' }, 1000));
    const [cmd] = ws.ofType('cmd');
    assert.deepEqual({ action: cmd.action, params: cmd.params }, { action: 'scroll', params: { direction: 'down' } });
    pendingCommands.settle(cmd.id, BROWSER, { ok: true });
    assert.deepEqual(await answer, { ok: true });
  });

  it('fails at once when the socket cannot be written', async () => {
    const ws = new FakeSocket();
    ws.failWith = new Error('socket closed');
    await assert.rejects(new SocketTransport(ws).send(describeCall(BROWSER, 'click', {}, 1000)), /socket closed/);
  });
});
