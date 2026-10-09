/** Native cookie operations retain ownership through awaits and never manufacture unsupported metadata. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { nativeCookies, cookieBatch } from '../../../../src/main/native-cookies/index.ts';
import { NativeProtocol } from '../../../../src/main/native-front-door/protocol.ts';
/** A canonical native cookie with explicit provenance from the engine. */
const cookie = {
  name: 'n',
  value: 'v',
  domain: 'example.test',
  path: '/',
  secure: false,
  httpOnly: true,
  session: true,
  sameSite: 'unspecified',
  _oyaPriority: 2,
  _oyaSourceScheme: 1,
  _oyaSourcePort: 8080,
  _oyaPartitioned: false,
};
test('cookie reads preserve native priority, provenance, session lifetime and unspecified SameSite', async () => {
  const get = () => ({ cookies: { get: async () => [cookie] } }) as any;
  const { cookies } = (await nativeCookies(get, 'cookies:get', {})) as any;
  assert.deepEqual(cookies, [
    {
      name: 'n',
      value: 'v',
      domain: 'example.test',
      path: '/',
      secure: false,
      httpOnly: true,
      session: true,
      expires: -1,
      size: 2,
      priority: 'High',
      sourceScheme: 'NonSecure',
      sourcePort: 8080,
    },
  ]);
});
test('missing engine metadata and partitioned cookies fail rather than returning invented attributes', async () => {
  for (const changed of [{ _oyaPriority: undefined }, { _oyaPartitioned: true }]) {
    const get = () => ({ cookies: { get: async () => [{ ...cookie, ...changed }] } }) as any;
    await assert.rejects(nativeCookies(get, 'cookies:get', {}), /metadata|Partitioned/);
  }
});
test('revocation during a read prevents returned cookie data from reaching the client', async () => {
  let authorized = true;
  const session = {
    cookies: {
      get: async () => {
        authorized = false;
        return [cookie];
      },
    },
  } as any;
  const get = () => {
    if (!authorized) throw Error('revoked');
    return session;
  };
  await assert.rejects(nativeCookies(get, 'cookies:get', {}), /revoked/);
});
test('the entire cookie batch is validated before the first native mutation', async () => {
  let writes = 0;
  const get = () =>
    ({
      cookies: {
        set: async () => {
          writes++;
        },
      },
    }) as any;
  await assert.rejects(
    nativeCookies(get, 'cookies:set', {
      cookies: [
        { url: 'https://example.test', name: 'ok', value: 'v' },
        { url: 'https://example.test', name: 'bad', value: 'v', partitionKey: {} },
      ],
    }),
    /Unsupported/,
  );
  assert.equal(writes, 0);
});
test('native rejection stops the batch and cannot be reported as success', async () => {
  let writes = 0;
  const get = () =>
    ({
      cookies: {
        set: async () => {
          writes++;
          throw Error('native rejected');
        },
      },
    }) as any;
  const value = { url: 'https://example.test', name: 'n', value: 'v' };
  await assert.rejects(nativeCookies(get, 'cookies:set', { cookies: [value, value] }), /native rejected/);
  assert.equal(writes, 1);
});
test('revocation after a write stops all later batch entries', async () => {
  let authorized = true,
    writes = 0;
  const session = {
    cookies: {
      set: async () => {
        writes++;
        authorized = false;
      },
    },
  } as any;
  const get = () => {
    if (!authorized) throw Error('revoked');
    return session;
  };
  const value = { url: 'https://example.test', name: 'n', value: 'v' };
  await assert.rejects(nativeCookies(get, 'cookies:set', { cookies: [value, value] }), /revoked/);
  assert.equal(writes, 1);
});
test('URL schemes, credentialed URLs, controls and unsupported cookie attributes are rejected', () => {
  const base = { url: 'https://example.test', name: 'n', value: 'v' };
  for (const value of [
    { url: 'file:///tmp/a' },
    { url: 'https://user:password@example.test' },
    { value: '\n' },
    { priority: 'High' },
    { expires: NaN },
    { secure: 'true' },
    { path: 'relative' },
  ])
    assert.throws(() => cookieBatch([{ ...base, ...value }]));
});
test('page sockets and attached sessions cannot invoke browser-wide cookie commands', async () => {
  let calls = 0;
  const backend: any = {
    targets: () => [{ targetId: 't' }],
    manage: async () => {
      calls++;
      return {};
    },
  };
  const direct = new NativeProtocol(backend, 't');
  for (const method of ['Storage.getCookies', 'Storage.clearCookies', 'Oya.deleteCookie'])
    await assert.rejects(
      direct.dispatch({
        id: 1,
        method,
        params:
          method === 'Oya.deleteCookie'
            ? { browserContextId: 'c', url: 'https://example.test', name: 'n' }
            : { browserContextId: 'c' },
      }),
      /browser endpoint/,
    );
  const browser = new NativeProtocol(backend);
  const { sessionId } = (await browser.dispatch({
    id: 2,
    method: 'Target.attachToTarget',
    params: { targetId: 't', flatten: true },
  })) as any;
  await assert.rejects(
    browser.dispatch({ id: 3, method: 'Storage.getCookies', params: { browserContextId: 'c' }, sessionId }),
    /browser endpoint/,
  );
  browser.dispose();
  assert.equal(calls, 0);
  direct.dispose();
});
