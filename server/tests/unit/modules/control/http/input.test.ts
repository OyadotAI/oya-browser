/**
 * Unit tests for humanInput, POST /control/sessions/:id/input: only the
 * browser actions a person may send, dispatched as that person's command.
 */
import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../../support/data-dir.ts';

ownDataDir('oya-control-http-input-');
const { humanInput } = await import('../../../../../src/modules/control/http/input.ts');
const { pendingCommands } =
  await import('../../../../../src/modules/browsers/connection/transports/pending-commands.ts');
const { connectBrowser, disconnectBrowser } = await import('../../../support/fakes.ts');

afterEach(() => disconnectBrowser('b-input'));

/** Lets the command reach the socket. */
const flush = () => new Promise((resolve) => setImmediate(resolve));

describe('humanInput', () => {
  it('refuses an action a person may not send', async () => {
    await assert.rejects(humanInput({ body: { action: 'evaluate' }, params: { id: 'b-input' } }), {
      status: 400,
      code: 'invalid_action',
    });
    await assert.rejects(humanInput({ params: { id: 'b-input' } }), { status: 400 });
  });

  it('sends the input to the browser and returns its result', async () => {
    const ws = connectBrowser('b-input');
    const answer = humanInput({
      authToken: 'oya_alice',
      params: { id: 'b-input' },
      body: { action: 'click', params: { x: 1 } },
    });
    await flush();
    const [command] = ws.sent.filter((m) => m.action === 'click');
    assert.deepEqual(command.params, { x: 1 });
    pendingCommands.settle(command.id, 'b-input', { ok: true, data: 'clicked' });
    assert.deepEqual(await answer, { ok: true, data: 'clicked' });
  });
});
