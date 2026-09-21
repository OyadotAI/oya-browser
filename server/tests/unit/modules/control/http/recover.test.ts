/**
 * Unit tests for recover, POST /control/sessions/:id/recover: a ready session
 * is answered as itself; a replacement needs an explicit request, an ended
 * original, and for CDP an endpoint. Starting the replacement goes through the
 * app router and is covered by the integration suites.
 */
import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../../support/data-dir.ts';

ownDataDir('oya-control-http-recover-');
const { control } = await import('../../../../../src/modules/control/service.ts');
const { recover } = await import('../../../../../src/modules/control/http/recover.ts');
const { readySession } = await import('../../../support/control.ts');

let n = 0,
  id;
beforeEach(async () => {
  id = `r-${n++}`;
  await readySession(control(), 'key-a', id);
});

/** A recover request from key-a. */
const request = (body = {}, key = 'key-a') => ({ principal: { key }, params: { id }, body });
/** A response that records its JSON. */
const response = () => {
  const res: any = {};
  res.json = (body) => ((res.body = body), res);
  return res;
};

describe('recover', () => {
  it('answers a ready session as itself', async () => {
    const res = response();
    await recover(request(), res);
    assert.equal(res.body.outcome, 'original_session');
    assert.equal(res.body.session.id, id);
  });

  it('requires an explicit replacement once the original is unavailable', async () => {
    await control().cancel('key-a', id, { force: true });
    await assert.rejects(recover(request(), response()), { status: 409, code: 'recovery_unavailable' });
  });

  it('refuses to replace an original that has not been cleaned up', async () => {
    await control().update('key-a', id, { state: 'disconnected' });
    await assert.rejects(recover(request({ replace: true }), response()), { code: 'cleanup_required' });
  });

  it('requires a CDP replacement to name its endpoint', async () => {
    await control().cancel('key-a', id, { force: true });
    await assert.rejects(recover(request({ replace: true }), response()), {
      status: 422,
      code: 'replacement_endpoint_required',
    });
  });

  it('answers 404 for another project’s session', async () => {
    await assert.rejects(recover(request({}, 'key-b'), response()), { status: 404 });
  });
});
