/**
 * Unit tests for PendingCommands: settling, timing out and failing the commands
 * sent over browser sockets.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { pendingCommands } from '../../../../../src/modules/browsers/connection/transports/pending-commands.ts';
import { describeCall } from '../../../../../src/modules/browsers/connection/reporter.ts';
import { connectBrowser, disconnectBrowser } from '../../../support/fakes.ts';

const BROWSER = 'b-pending';

describe('PendingCommands', () => {
  beforeEach(() => {
    mock.timers.enable({ apis: ['setTimeout'] });
    connectBrowser(BROWSER);
  });
  afterEach(() => {
    mock.timers.reset();
    disconnectBrowser(BROWSER);
  });

  it('resolves a command with the result that answers it', async () => {
    const answer = pendingCommands.wait('c1', describeCall(BROWSER, 'click', {}, 1000));
    assert.equal(pendingCommands.settle('c1', BROWSER, { ok: true, data: { clicked: true } }), true);
    assert.deepEqual(await answer, { ok: true, data: { clicked: true } });
  });

  it('ignores a result from another browser', async () => {
    const answer = pendingCommands.wait('c2', describeCall(BROWSER, 'click', {}, 1000));
    assert.equal(pendingCommands.settle('c2', 'someone-else', { ok: true }), false);
    pendingCommands.settle('c2', BROWSER, { ok: false, error: 'late' });
    assert.equal((await answer).error, 'late');
  });

  it('rejects once the timeout passes, naming the command and the wait', async () => {
    const answer = pendingCommands.wait('c3', describeCall(BROWSER, 'navigate', {}, 5000));
    mock.timers.tick(5000);
    await assert.rejects(answer, /Command navigate timed out after 5s/);
    assert.equal(pendingCommands.settle('c3', BROWSER, { ok: true }), false, 'a late answer finds nothing');
  });

  it('fails every command on a browser that disconnected, and only that browser', async () => {
    connectBrowser('other');
    const mine = pendingCommands.wait('c4', describeCall(BROWSER, 'type', {}, 1000));
    const theirs = pendingCommands.wait('c5', describeCall('other', 'type', {}, 1000));
    pendingCommands.failBrowser(BROWSER, 'disconnected', 'Browser disconnected');
    await assert.rejects(mine, /Browser disconnected/);
    assert.equal(pendingCommands.settle('c5', 'other', { ok: true }), true);
    assert.deepEqual(await theirs, { ok: true });
    disconnectBrowser('other');
  });
});
