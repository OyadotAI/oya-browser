/**
 * Unit tests for connection tickets: single-use, one-minute stand-ins for a
 * credential on one session's connection URL.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { hash } from '../../../../../src/modules/control/service.ts';
import { scratchService } from '../../../support/control.ts';

const NOW = 1_700_000_000_000;
let service;
beforeEach(() => {
  mock.timers.enable({ apis: ['Date'], now: NOW });
  service = scratchService();
});
afterEach(() => mock.timers.reset());

describe('ticket and redeem', () => {
  it('redeems a ticket for the credential it stands for', async () => {
    const t = await service.ticket('key-a', 's1', 'oya_token');
    assert.equal(await service.redeem(t, 's1'), 'oya_token');
  });

  it('stands in for the project key when no other credential is given', async () => {
    assert.equal(await service.redeem(await service.ticket('key-a', 's1'), 's1'), 'key-a');
  });

  it('stores the ticket under its hash with the credential sealed', async () => {
    const t = await service.ticket('key-a', 's1', 'oya_token');
    const row = await service.store.get('ticket', hash(t));
    assert.equal(row.sessionId, 's1');
    assert.equal(row.expiresAt, NOW + 60_000);
    assert.equal(JSON.stringify(row).includes('oya_token'), false);
  });

  it('works only once', async () => {
    const t = await service.ticket('key-a', 's1');
    await service.redeem(t, 's1');
    await assert.rejects(service.redeem(t, 's1'), { status: 401, code: 'invalid_ticket' });
  });

  it('refuses a ticket for another session', async () => {
    const t = await service.ticket('key-a', 's1');
    await assert.rejects(service.redeem(t, 's2'), { status: 401 });
  });

  it('refuses a ticket after its minute is up', async () => {
    const t = await service.ticket('key-a', 's1');
    mock.timers.tick(60_001);
    await assert.rejects(service.redeem(t, 's1'), { status: 401 });
  });
});
