/** The Oya calls: the key goes only to oyabrowser.com, as a bearer token, and refusals carry their status. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { OyaError, oyaClient } from '../../src/oya.js';
import { fakeFetch, offlineFetch } from './support/fakes.ts';

test('every call goes to oyabrowser.com with the key as a bearer token', async () => {
  const { fetchFn, requests } = fakeFetch({ 'GET /api/personas': { status: 200, body: { personas: [] } } });
  await oyaClient('secret', fetchFn).listProfiles();
  assert.equal(new URL(requests[0].url).origin, 'https://oyabrowser.com');
  assert.equal(requests[0].headers.Authorization, 'Bearer secret');
});

test('a refusal carries the status and the server reason', async () => {
  const { fetchFn } = fakeFetch({ 'GET /api/auth/whoami': { status: 403, body: { error: 'Invalid API key' } } });
  const error = await oyaClient('k', fetchFn)
    .whoami()
    .catch((e) => e);
  assert.ok(error instanceof OyaError);
  assert.equal(error.status, 403);
  assert.equal(error.reason, 'Invalid API key');
});

test('Oya unreachable is status 0', async () => {
  const error = await oyaClient('k', offlineFetch)
    .listProfiles()
    .catch((e) => e);
  assert.equal(error.status, 0);
});

test('the profile id is escaped into the import address', async () => {
  const { fetchFn, requests } = fakeFetch({ 'PUT /api/pool/cookies': { status: 200, body: {} } });
  await oyaClient('k', fetchFn).sendCookies('a b&c', []);
  assert.equal(requests[0].url, 'https://oyabrowser.com/api/pool/cookies?persona=a%20b%26c');
});
