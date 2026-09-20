/**
 * Unit tests for mailbox access tokens: minted from a refresh token at Google
 * or Microsoft, cached until a minute before expiry, and a refusal reported
 * with the provider's reason.
 */
import { describe, it, afterEach, beforeEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { accessToken, clearTokens } from '../../../../src/modules/challenges/mailbox-token.ts';
import { TOKEN_REFRESH_MARGIN_MS } from '../../../../src/modules/challenges/constants.ts';
import { Status } from '../../../../src/platform/http-status.ts';
import { json, stubFetch, text } from '../../support/http.ts';

describe('accessToken', () => {
  beforeEach(() => {
    clearTokens();
    mock.timers.enable({ apis: ['Date'], now: 0 });
  });
  afterEach(() => {
    mock.restoreAll();
    mock.timers.reset();
  });

  it('mints a Gmail token at Google with the refresh-token grant', async () => {
    const calls = stubFetch(() => json({ access_token: 'g-1', expires_in: 3600 }));
    const token = await accessToken('gmail', { refreshToken: 'rt', clientId: 'cid', clientSecret: 'cs' });
    assert.equal(token, 'g-1');
    assert.equal(calls[0].url, 'https://oauth2.googleapis.com/token');
    const form = new URLSearchParams(calls[0].init.body);
    assert.deepEqual(
      [form.get('grant_type'), form.get('refresh_token'), form.get('client_id'), form.get('client_secret')],
      ['refresh_token', 'rt', 'cid', 'cs'],
    );
    assert.equal(form.get('scope'), null);
  });

  it('mints a Graph token at the tenant’s Microsoft endpoint with the mail scope', async () => {
    const calls = stubFetch(() => json({ access_token: 'm-1' }));
    await accessToken('graph', { refreshToken: 'rt', clientId: 'cid', tenant: 'contoso.com' });
    assert.equal(calls[0].url, 'https://login.microsoftonline.com/contoso.com/oauth2/v2.0/token');
    const form = new URLSearchParams(calls[0].init.body);
    assert.match(form.get('scope')!, /Mail\.Read/);
    assert.equal(form.get('client_secret'), null, 'no secret is sent when none is configured');
  });

  it('uses the common tenant when none is configured', async () => {
    const calls = stubFetch(() => json({ access_token: 'm-1' }));
    await accessToken('graph', { refreshToken: 'rt', clientId: 'cid' });
    assert.match(calls[0].url, /\/common\/oauth2/);
  });

  it('reuses a cached token until a minute before it expires', async () => {
    const calls = stubFetch(() => json({ access_token: `t-${calls.length}`, expires_in: 120 }));
    const config = { refreshToken: 'rt', clientId: 'cid' };
    assert.equal(await accessToken('gmail', config), 't-1');
    mock.timers.tick(120_000 - TOKEN_REFRESH_MARGIN_MS - 1);
    assert.equal(await accessToken('gmail', config), 't-1');
    mock.timers.tick(1);
    assert.equal(await accessToken('gmail', config), 't-2');
  });

  it('forgets every cached token when cleared', async () => {
    const calls = stubFetch(() => json({ access_token: 'x' }));
    const config = { refreshToken: 'rt', clientId: 'cid' };
    await accessToken('gmail', config);
    clearTokens();
    await accessToken('gmail', config);
    assert.equal(calls.length, 2);
  });

  it('reports a refused refresh as 502 carrying the provider’s reason', async () => {
    stubFetch(() => text('{"error":"invalid_grant"}', 400));
    await assert.rejects(accessToken('gmail', { refreshToken: 'rt', clientId: 'cid' }), {
      status: Status.BAD_GATEWAY,
      message: 'Mailbox token refresh failed (400) {"error":"invalid_grant"}',
    });
  });
});
