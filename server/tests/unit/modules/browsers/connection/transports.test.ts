/**
 * Unit tests for the command transport strategies: a driven (CDP) browser and
 * an Oya client socket.
 */
import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { CdpConnectionError } from '../../../../../src/drivers/cdp.ts';
import { DriverTransport } from '../../../../../src/modules/browsers/connection/transports/driver-transport.ts';
import { SocketTransport } from '../../../../../src/modules/browsers/connection/transports/socket-transport.ts';
import { pendingCommands } from '../../../../../src/modules/browsers/connection/transports/pending-commands.ts';
import { describeCall } from '../../../../../src/modules/browsers/connection/reporter.ts';
import { settleResult } from '../../../../../src/modules/browsers/connection/commands.ts';
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

  it('answers a page that refused the command as a failed result, not a thrown error', async () => {
    const driver = { isAlive: () => true, send: async () => Promise.reject(new Error('Element not found')) };
    const result = await new DriverTransport(driver).send(describeCall(BROWSER, 'click', {}, 1000));
    assert.deepEqual(result, { ok: false, error: 'Element not found', code: 'command_failed' });
  });

  it('answers a navigation the page could not make the same way, even when its words look like a timeout', async () => {
    const driver = {
      isAlive: () => true,
      send: async () => Promise.reject(new Error('Navigation failed: net::ERR_CONNECTION_TIMED_OUT')),
    };
    const result = await new DriverTransport(driver).send(describeCall(BROWSER, 'navigate', {}, 1000));
    assert.equal(result.ok, false);
    assert.equal(result.code, 'command_failed');
  });

  it('names the tab when the page has no such target', async () => {
    const driver = {
      isAlive: () => true,
      send: async () => Promise.reject(new Error('No target with given id found')),
    };
    const result = await new DriverTransport(driver).send(describeCall(BROWSER, 'switch_tab', { id: 't-9' }, 1000));
    assert.equal(result.error, 'Tab t-9 not found');
  });

  it('lets a lost connection through as a thrown error, so the outcome is reported as unknown', async () => {
    const lost = {
      isAlive: () => true,
      send: async () => Promise.reject(new CdpConnectionError('CDP connection closed')),
    };
    await assert.rejects(new DriverTransport(lost).send(describeCall(BROWSER, 'click', {}, 1000)), CdpConnectionError);
    const dead = { isAlive: () => false, send: async () => Promise.reject(new Error('anything')) };
    await assert.rejects(new DriverTransport(dead).send(describeCall(BROWSER, 'click', {}, 1000)), /anything/);
  });

  it('reports a dead engine’s plain error as a lost connection, so the caller hears outcome unknown and not a bug', async () => {
    const dead = { isAlive: () => false, send: async () => Promise.reject(new Error('Browser not connected')) };
    await assert.rejects(new DriverTransport(dead).send(describeCall(BROWSER, 'click', {}, 1000)), CdpConnectionError);
  });

  it('records a refused command as a failure in the activity log', async () => {
    connectBrowser(BROWSER);
    const driver = { isAlive: () => true, send: async () => Promise.reject(new Error('Element not found')) };
    await new DriverTransport(driver).send(describeCall(BROWSER, 'click', {}, 1000));
    assert.equal(registry.get(BROWSER).activity[0].ok, false);
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

  it('gives an Oya browser’s refusal a code, so both browser kinds answer a failed command the same way', async () => {
    const ws = connectBrowser(BROWSER);
    const answer = new SocketTransport(ws).send(describeCall(BROWSER, 'click', {}, 1000));
    const [cmd] = ws.ofType('cmd');
    settleResult(BROWSER, { id: cmd.id, ok: false, error: 'Element not found' }, true);
    assert.deepEqual(await answer, { ok: false, data: undefined, error: 'Element not found', code: 'command_failed' });
  });

  it('keeps a code the Oya browser names, and gives a success none', async () => {
    const ws = connectBrowser(BROWSER);
    const failed = new SocketTransport(ws).send(describeCall(BROWSER, 'navigate', {}, 1000));
    settleResult(BROWSER, { id: ws.ofType('cmd')[0].id, ok: false, error: 'x', code: 'tab_unprotected' }, true);
    assert.equal((await failed).code, 'tab_unprotected');
    const fine = new SocketTransport(ws).send(describeCall(BROWSER, 'click', {}, 1000));
    settleResult(BROWSER, { id: ws.ofType('cmd')[1].id, ok: true, data: { url: 'u' } }, true);
    assert.equal((await fine).code, undefined);
  });

  it('fails at once when the socket cannot be written', async () => {
    const ws = new FakeSocket();
    ws.failWith = new Error('socket closed');
    await assert.rejects(new SocketTransport(ws).send(describeCall(BROWSER, 'click', {}, 1000)), /socket closed/);
  });
});
