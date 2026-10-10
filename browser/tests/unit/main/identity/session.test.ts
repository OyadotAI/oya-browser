/**
 * Unit tests for src/main/identity/session.ts: the session's user agent and client hints
 * match the persona's platform and never mention Electron. Electron's app is faked.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { configureSession as configure } from '../../../../src/main/identity/session.ts';

const ELECTRON_UA =
  'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) oya-browser/1.0.97 Chrome/134.0.6998.44 Electron/35.1.2 Safari/537.36';

/** A session that records its user agent, header rewriter and proxy. */
function fakeSession(ua = ELECTRON_UA): any {
  return {
    ua,
    proxy: null,
    getUserAgent() {
      return this.ua;
    },
    setUserAgent(value: string, languages?: string) {
      this.ua = value;
      this.languages = languages;
    },
    setPermissionRequestHandler(fn: any) {
      this.permissionRequest = fn;
    },
    setPermissionCheckHandler(fn: any) {
      this.permissionCheck = fn;
    },
    async setProxy(value: any) {
      this.proxy = value;
    },
    webRequest: {
      onBeforeSendHeaders(fn: any) {
        this.rewrite = fn;
      },
      onHeadersReceived() {},
    },
  };
}

describe('configureSession', () => {
  const app = { userAgentFallback: '' };
  const configureSession = (ses: any, profile: any) => configure(app, ses, profile);

  it('applies the persona proxy, or goes direct without one', async () => {
    const withProxy = fakeSession();
    await configureSession(withProxy, { proxy: { host: 'gate.test', port: 8080 } });
    assert.equal(withProxy.proxy.proxyRules, 'http://gate.test:8080');
    const direct = fakeSession();
    await configureSession(direct, {});
    assert.deepEqual(direct.proxy, { mode: 'direct' });
  });
});

it('native browsing keeps the engine identity and headers without weakening permission checks', async () => {
  const ses = fakeSession();
  const app = { userAgentFallback: ELECTRON_UA };
  await configure(app, ses, null, { nativeBrowsing: true });
  assert.equal(ses.ua, ELECTRON_UA);
  assert.equal(app.userAgentFallback, ELECTRON_UA);
  assert.equal(ses.webRequest.rewrite, undefined);
  assert.equal(ses.permissionCheck(null, 'geolocation', 'https://example.com', {}), false);
});
it('native sessions install proxy and governance without a header-rewriting identity fallback', async () => {
  const ses = fakeSession();
  let installed = false;
  const governance: any = {
    configuration: { proxy: { host: 'managed.test', port: 8080 } },
    install(actual: any) {
      assert.equal(actual, ses);
      installed = true;
    },
  };
  await configure(
    { userAgentFallback: '' },
    ses,
    { proxy: { host: 'persona.test' } },
    { nativeBrowsing: true, governance },
  );
  assert.equal(ses.proxy.proxyRules, 'http://managed.test:8080');
  assert.equal(installed, true);
  assert.equal(ses.webRequest.rewrite, undefined);
});
