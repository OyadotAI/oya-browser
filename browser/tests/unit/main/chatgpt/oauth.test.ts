/**
 * Unit tests for the ChatGPT OAuth steps: the authorize address (registration on
 * a first sign-in only), the credential a token answer makes (nonce and plan
 * permission checked), and a refresh that keeps what the answer leaves out.
 */
import { it, mock } from 'node:test';
import assert from 'node:assert/strict';
import * as O from '../../../../src/main/chatgpt/oauth.ts';
import * as C from '../../../../src/main/chatgpt/constants.ts';

const PLAN = 'openid email offline_access chatgpt.tokens.use.direct';

/** An unsigned ID token carrying `claims`. */
const idToken = (claims: object) =>
  ['{"alg":"none"}', JSON.stringify(claims), 's'].map((p) => Buffer.from(p).toString('base64url')).join('.');

it('opens a first sign-in as a registration that names the app, a later one under the issued client', () => {
  const first = new URL(O.authorizeUrl(O.newLogin('http://127.0.0.1:5000/auth/callback'), 'urn:uuid:h'));
  assert.equal(first.searchParams.get('client_id'), 'dynamic_agent_client');
  assert.equal(first.searchParams.get('agent_name_hint'), 'Oya Browser');
  assert.equal(first.searchParams.get('code_challenge_method'), 'S256');
  assert.equal(first.searchParams.get('ext_agent_host_id'), 'urn:uuid:h');
  const later = new URL(O.authorizeUrl(O.newLogin('http://127.0.0.1:5000/auth/callback', 'oaiapp_x'), 'urn:uuid:h'));
  assert.deepEqual(
    [later.searchParams.get('client_id'), later.searchParams.get('agent_name_hint')],
    ['oaiapp_x', null],
  );
});

it('makes a credential only for the sign-in it answers, and only with the plan permission', () => {
  const tokens = {
    access_token: 'a',
    refresh_token: 'r',
    expires_in: 3600,
    scope: PLAN,
    id_token: idToken({ nonce: 'n', email: 'me@x.test' }),
  };
  const credential = O.credentialFrom(tokens, 'oaiapp_x', 'n');
  assert.deepEqual([credential.client_id, credential.email, credential.refresh_token], ['oaiapp_x', 'me@x.test', 'r']);
  assert.throws(() => O.credentialFrom(tokens, 'oaiapp_x', 'other'), /did not match/);
  assert.throws(() => O.credentialFrom({ ...tokens, scope: 'openid email' }, 'oaiapp_x', 'n'), {
    message: C.NO_PLAN_SCOPE,
  });
});

it('a refresh keeps the refresh token and email the answer leaves out', async () => {
  const fake = mock.fn(async () => new Response(JSON.stringify({ access_token: 'a2', expires_in: 3600 })));
  const next = await O.refreshed({ client_id: 'oaiapp_x', refresh_token: 'r', email: 'me@x.test' }, fake as never);
  assert.deepEqual([next.access_token, next.refresh_token, next.email], ['a2', 'r', 'me@x.test']);
  const form = new URLSearchParams(String((fake.mock.calls[0].arguments as any[])[1].body));
  assert.deepEqual([form.get('grant_type'), form.get('client_id')], ['refresh_token', 'oaiapp_x']);
});

it("a refused refresh keeps OpenAI's code and never names a plan", async (t) => {
  t.mock.method(console, 'warn', () => {});
  const fake = mock.fn(async () => new Response(JSON.stringify({ error: 'invalid_grant' }), { status: 400 }));
  const error = await O.refreshed({ client_id: 'oaiapp_x', refresh_token: 'r' }, fake as never).catch((e) => e);
  assert.match(error.message, /invalid_grant/);
  assert.doesNotMatch(error.message, /Plus or Pro/);
});
