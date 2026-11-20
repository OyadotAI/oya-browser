/**
 * Unit tests for the mounted OAuth endpoints: discovery, registration, the
 * redirect to the console, and a PKCE code exchange end to end over HTTP; and
 * that a server without an HTTPS (or localhost) origin keeps OAuth off.
 */
import { describe, it, after } from 'node:test';
import assert from 'node:assert/strict';
import { createHash, randomBytes } from 'node:crypto';
import express from 'express';
import { ownDataDir, restoreEnv } from '../../support/data-dir.ts';

ownDataDir('oya-mcp-oauth-router-');
const saved = process.env.OYA_CONSOLE_URL;
after(() => restoreEnv('OYA_CONSOLE_URL', saved));
const { oauthRouter, challenge } = await import('../../../../src/mcp/oauth/router.ts');
const { allow } = await import('../../../../src/mcp/oauth/connect.ts');
const { control, projectId } = await import('../../../../src/modules/control/service.ts');
const { authenticateToken } = await import('../../../../src/modules/auth/service.ts');
const { patchRow } = await import('../../support/control.ts');

const REDIRECT = 'https://claude.ai/api/mcp/auth_callback';
/** Registrations the MCP SDK allows one address per hour before answering 429. */
const PER_ADDRESS_REGISTRATIONS = 20;

/** Runs `work` against the OAuth router on a throwaway loopback server. */
async function withServer(work: (base: string) => Promise<void>) {
  const server = express().use(oauthRouter()!).listen(0);
  try {
    await work(`http://127.0.0.1:${(server.address() as any).port}`);
  } finally {
    server.close();
  }
}

/** A PKCE verifier and its S256 challenge. */
function pkce() {
  const verifier = randomBytes(32).toString('base64url');
  return { verifier, challenge: createHash('sha256').update(verifier).digest('base64url') };
}

/** Registers a public client the way claude.ai does. */
async function register(base: string) {
  const body = { client_name: 'Claude', redirect_uris: [REDIRECT], token_endpoint_auth_method: 'none' };
  const res = await fetch(`${base}/register`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
  });
  assert.equal(res.status, 201);
  return res.json();
}

/** Starts a sign-in; the console URL it redirects to. */
async function authorize(base: string, clientId: string, codeChallenge: string) {
  const query = new URLSearchParams({
    response_type: 'code',
    client_id: clientId,
    redirect_uri: REDIRECT,
    code_challenge: codeChallenge,
    code_challenge_method: 'S256',
    state: 'st',
  });
  const res = await fetch(`${base}/authorize?${query}`, { redirect: 'manual' });
  assert.equal(res.status, 302);
  return new URL(res.headers.get('location')!);
}

/** Exchanges a code at /token. */
function token(base: string, clientId: string, code: string, verifier: string) {
  const body = new URLSearchParams({
    grant_type: 'authorization_code',
    client_id: clientId,
    code,
    code_verifier: verifier,
    redirect_uri: REDIRECT,
  });
  return fetch(`${base}/token`, { method: 'POST', body });
}

describe('OAuth router', () => {
  it('stays off when the public origin is plain HTTP elsewhere', () => {
    process.env.OYA_CONSOLE_URL = 'http://oya.internal';
    assert.equal(oauthRouter(), null);
    assert.equal(challenge(), '');
  });

  it('advertises the pool as the protected resource', async () => {
    process.env.OYA_CONSOLE_URL = 'https://oya.example';
    await withServer(async (base) => {
      const meta = await (await fetch(`${base}/.well-known/oauth-protected-resource/mcp/pool`)).json();
      assert.equal(meta.resource, 'https://oya.example/mcp/pool');
      assert.deepEqual(meta.authorization_servers, ['https://oya.example/']);
    });
    assert.equal(
      challenge(),
      'Bearer resource_metadata="https://oya.example/.well-known/oauth-protected-resource/mcp/pool"',
    );
  });

  it('never redirects a malformed /authorize to the client: that would make this origin a redirector', async () => {
    process.env.OYA_CONSOLE_URL = 'https://oya.example';
    await withServer(async (base) => {
      const client = await register(base);
      const bare = new URLSearchParams({ client_id: client.client_id, redirect_uri: REDIRECT });
      const res = await fetch(`${base}/authorize?${bare}`, { redirect: 'manual' });
      assert.equal(res.status, 400);
      assert.equal((await res.json()).error, 'invalid_request');
      const plain = new URLSearchParams({
        ...Object.fromEntries(bare),
        response_type: 'code',
        code_challenge: 'c',
        code_challenge_method: 'plain',
      });
      assert.equal((await fetch(`${base}/authorize?${plain}`, { redirect: 'manual' })).status, 400);
    });
  });

  it('never sends the browser to a client address, even for a refusal the SDK would redirect', async () => {
    process.env.OYA_CONSOLE_URL = 'https://oya.example';
    await withServer(async (base) => {
      const client = await register(base);
      const query = new URLSearchParams({
        response_type: 'code',
        client_id: client.client_id,
        redirect_uri: REDIRECT,
        code_challenge: 'c',
        code_challenge_method: 'S256',
        resource: 'not a url',
      });
      const res = await fetch(`${base}/authorize?${query}`, { redirect: 'manual' });
      assert.equal(res.status, 400);
      assert.equal(res.headers.get('location'), null);
    });
  });

  it('answers a body that is not JSON as OAuth does, not with an HTML stack trace', async () => {
    process.env.OYA_CONSOLE_URL = 'https://oya.example';
    await withServer(async (base) => {
      const res = await fetch(`${base}/register`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: 'nope',
      });
      assert.equal(res.status, 400);
      assert.equal((await res.json()).error, 'invalid_request');
    });
  });

  it('limits registrations per caller, so one address running out leaves everyone else able to sign in', async () => {
    process.env.OYA_CONSOLE_URL = 'https://oya.example';
    /** Registers once as if forwarded from `address`; the status it got. */
    const registerFrom = async (base: string, address: string) => {
      const body = { client_name: 'Claude', redirect_uris: [REDIRECT], token_endpoint_auth_method: 'none' };
      const res = await fetch(`${base}/register`, {
        method: 'POST',
        headers: { 'content-type': 'application/json', 'x-forwarded-for': address },
        body: JSON.stringify(body),
      });
      return res.status;
    };
    await withServer(async (base) => {
      const statuses = [];
      for (let i = 0; i <= PER_ADDRESS_REGISTRATIONS; i++) statuses.push(await registerFrom(base, '198.51.100.7'));
      assert.equal(statuses.at(-1), 429, 'the next registration past the per-address limit is refused');
      assert.ok(statuses.slice(0, -1).every((status) => status === 201));
      assert.equal(await registerFrom(base, '198.51.100.8'), 201, 'another caller still registers');
    });
  });

  it('refuses to register a redirect URI the console could not safely follow', async () => {
    process.env.OYA_CONSOLE_URL = 'https://oya.example';
    await withServer(async (base) => {
      for (const uri of ['javascript:alert(1)', 'data:text/html,x', 'http://evil.example/cb']) {
        const body = JSON.stringify({ client_name: 'x', redirect_uris: [uri], token_endpoint_auth_method: 'none' });
        const res = await fetch(`${base}/register`, {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
          body,
        });
        assert.equal(res.status, 400, uri);
      }
    });
  });

  it('signs a client in with PKCE: register, console, allow, token', async () => {
    process.env.OYA_CONSOLE_URL = 'https://oya.example';
    await control().project('router-key');
    await patchRow(control(), 'project', projectId('router-key'), { ownerUser: 'router-owner' });
    await withServer(async (base) => {
      const client = await register(base);
      const { verifier, challenge: codeChallenge } = pkce();
      const consolePage = await authorize(base, client.client_id, codeChallenge);
      assert.equal(consolePage.origin + consolePage.pathname, 'https://oya.example/connect');
      const allowed = await allow(consolePage.searchParams.get('request')!, 'router-owner', projectId('router-key'));
      const code = new URL(allowed.redirect).searchParams.get('code')!;
      assert.equal((await token(base, client.client_id, code, 'wrong-verifier')).status, 400);
      const res = await token(base, client.client_id, code, verifier);
      assert.equal(res.status, 200);
      assert.equal((await authenticateToken((await res.json()).access_token)).key, 'router-key');
    });
  });
});
