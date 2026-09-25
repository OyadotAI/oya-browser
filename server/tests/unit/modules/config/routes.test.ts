/**
 * Unit tests for the config routes: a key reads and writes its own settings,
 * and only the operator sets the deployment-wide default.
 */
import { describe, it, before, after, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir();
const { router } = await import('../../../../src/modules/config/routes.ts');
const { runtimeConfig } = await import('../../../../src/platform/runtime-config.ts');
const { allowKey, callRoute } = await import('../../support/agent.ts');
const keyConfig = await import('../../../../src/modules/config/service.ts');
const { registry } = await import('../../../../src/modules/browsers/registry.ts');

const KEY = 'config-routes-key';
const OPERATOR = 'config-operator-token';
let forget: () => void;

describe('config routes', () => {
  before(() => {
    forget = allowKey(KEY);
    process.env.OYA_OPERATOR_TOKEN = OPERATOR;
  });
  after(() => {
    forget();
    delete process.env.OYA_OPERATOR_TOKEN;
  });

  it('tells the key’s desktop apps when its model changes, and not for other settings', async () => {
    const tell = mock.method(registry, 'tell', () => 0);
    const post = (body) => callRoute(router, { method: 'POST', url: '/config', key: KEY, body });
    await post({ chat_model: 'm2' });
    assert.deepEqual(tell.mock.calls[0].arguments, [KEY, { type: 'settings_changed', scope: 'llm' }]);
    await post({ captcha_solver: '2captcha' });
    assert.equal(tell.mock.callCount(), 1);
    tell.mock.restore();
  });

  it('refuses a caller without an API key', async () => {
    assert.equal((await callRoute(router, { url: '/config' })).status, 401);
  });

  it('saves a key’s settings and answers with the masked view', async () => {
    const res = await callRoute(router, {
      method: 'POST',
      url: '/config',
      key: KEY,
      body: { openai_api_key: 'sk-routes-9999', chat_model: 'm1' },
    });
    assert.equal(res.body.ok, true);
    assert.equal(res.body.openai_api_key, '••••9999');
    const read = await callRoute(router, { url: '/config', key: KEY });
    assert.equal(read.body.chat_model, 'm1');
  });

  it('shows the notes the agent kept for a site, and forgets one when told to', async () => {
    await keyConfig.saveSiteNotes(KEY, 'shop.test', ['exports live under Account']);
    const read = await callRoute(router, { url: '/config/site-notes', key: KEY });
    assert.deepEqual(read.body.notes, { 'shop.test': ['exports live under Account'] });
    const gone = await callRoute(router, { method: 'DELETE', url: '/config/site-notes/shop.test', key: KEY });
    assert.deepEqual(gone.body, { ok: true, notes: {} });
  });

  it('answers an invalid setting with 400', async () => {
    const res = await callRoute(router, { method: 'POST', url: '/config', key: KEY, body: { llm_provider: 'x' } });
    assert.equal(res.status, 400);
  });

  it('refuses the host default to a tenant key', async () => {
    const res = await callRoute(router, { method: 'POST', url: '/config/host', key: KEY, body: { chat_model: 'x' } });
    assert.equal(res.status, 403);
  });

  it('sets the host default for the operator', async () => {
    const res = await callRoute(router, {
      method: 'POST',
      url: '/config/host',
      key: OPERATOR,
      body: { chat_model: 'host-model' },
    });
    assert.equal(res.body.scope, 'host');
    assert.equal(runtimeConfig.getChatModel(), 'host-model');
  });
});
