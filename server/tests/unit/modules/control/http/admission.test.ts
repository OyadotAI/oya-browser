/**
 * Unit tests for the steps of HTTP admission: reserving with the request's
 * limits, answering a replay (with a fresh ticket in place of any token), and
 * persisting the start handler's answer before it is sent.
 */
import { describe, it, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../../support/data-dir.ts';

ownDataDir('oya-control-http-admission-');
const { control } = await import('../../../../../src/modules/control/service.ts');
const { answerReplay, persistOutcome, reserve, starting } =
  await import('../../../../../src/modules/control/http/admission.ts');

let n = 0;
/** A fresh session id. */
const nextId = () => `adm-${n++}`;

/** An Express-like request with an optional Idempotency-Key. */
const request = (body: any = {}, idempotency?: string) => ({
  body,
  get: (name) => (name === 'Idempotency-Key' ? idempotency : undefined),
});

/** A response that records its status and JSON. */
function response() {
  const res: any = { statusCode: 200 };
  res.status = (code) => ((res.statusCode = code), res);
  res.json = (body) => ((res.body = body), res);
  return res;
}

describe('reserve', () => {
  it('reserves for the request, marking a server-run provider managed', async () => {
    const id = nextId();
    const x = await reserve('key-a', request({ id }), 'oya-selfhosted', { id: 'p1', maxConcurrent: 2 });
    assert.equal(x.managed, true);
    assert.equal(x.persona, 'p1');
    assert.equal(x.provider, 'oya-selfhosted');
    assert.equal((await reserve('key-a', request(), 'cdp', null)).managed, false);
  });

  it('replays by Idempotency-Key', async () => {
    const first = await reserve('key-a', request({ a: 1 }, 'idem-1'), 'cdp', null);
    const again = await reserve('key-a', request({ a: 1 }, 'idem-1'), 'cdp', null);
    assert.equal(again.id, first.id);
    assert.equal(again.replay, true);
  });
});

describe('starting', () => {
  it('is the 202 body for a start that is not ready yet', () => {
    assert.deepEqual(starting('s', 'cdp', 'p', 'queued'), {
      id: 's',
      operationId: 's',
      provider: 'cdp',
      persona: 'p',
      status: 'starting',
      state: 'queued',
    });
  });
});

describe('answerReplay', () => {
  it('answers 202 while the original start has no outcome yet', async () => {
    const res = response();
    await answerReplay('key-a', {}, res, { id: 's', provider: 'cdp', persona: null, state: 'provisioning' });
    assert.equal(res.statusCode, 202);
    assert.equal(res.body.status, 'starting');
  });

  it('answers the stored outcome, swapping any token in the CDP URL for a fresh ticket', async () => {
    const res = response();
    const reservation = {
      id: 's',
      response: { status: 201, body: { cdpUrl: 'wss://h.example/cdp/s?token=secret', id: 's' } },
    };
    await answerReplay('key-a', { authToken: 'oya_cred' }, res, reservation);
    assert.equal(res.statusCode, 201);
    const url = new URL(res.body.cdpUrl);
    assert.equal(url.searchParams.get('token'), null);
    assert.equal(await control().redeem(url.searchParams.get('ticket'), 's'), 'oya_cred');
    assert.equal(reservation.response.body.cdpUrl, 'wss://h.example/cdp/s?token=secret', 'stored outcome untouched');
  });

  it('answers a stored outcome without a CDP URL as it was', async () => {
    const res = response();
    await answerReplay('key-a', {}, res, { id: 's', response: { status: 400, body: { error: 'x' } } });
    assert.deepEqual([res.statusCode, res.body], [400, { error: 'x' }]);
  });
});

describe('persistOutcome', () => {
  it('stores the handler’s answer as the session’s outcome before sending it', async () => {
    const x = await control().reserve('key-a', { id: nextId(), provider: 'cdp' });
    const res = response();
    const sent = new Promise((resolve) => {
      const json = res.json;
      res.json = (body) => (json(body), resolve(body), res);
    });
    persistOutcome('key-a', x, res);
    res.statusCode = 200;
    res.json({ ok: true });
    assert.deepEqual(await sent, { ok: true });
    assert.equal((await control().store.get('session', x.id)).state, 'ready');
  });

  it('answers 503 storage_unavailable when the outcome cannot be stored', async () => {
    const res = response();
    const sent = new Promise((resolve) => {
      const json = res.json;
      res.json = (body) => (json(body), resolve(body), res);
    });
    const complete = mock.method(control(), 'complete', async () => {
      throw new Error('down');
    });
    persistOutcome('key-a', { id: 'op-1' }, res);
    res.json({ ok: true });
    assert.deepEqual(await sent, {
      error: 'Could not persist operation outcome',
      code: 'storage_unavailable',
      operationId: 'op-1',
    });
    assert.equal(res.statusCode, 503);
    complete.mock.restore();
  });
});
