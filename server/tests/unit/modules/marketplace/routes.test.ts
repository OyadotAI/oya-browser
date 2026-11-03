/** In-process HTTP routes exercise AWS form parsing, authentication and explicit account consent. */
import { Readable } from 'node:stream';
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { marketplaceCallback } from '../../../../src/modules/marketplace/callback.ts';
import { marketplaceRoutes } from '../../../../src/modules/marketplace/routes.ts';
import { MarketplaceService } from '../../../../src/modules/marketplace/service.ts';
import { openContext } from '../../../../src/modules/marketplace/continuation.ts';
import { sessionUsers } from '../../../../src/modules/auth/middleware.ts';
import { FakeResponse, fakeRequest, routeThrough } from '../../support/auth.ts';

process.env.OYA_PROFILE_SECRET = 'marketplace-routes-hermetic-key-only';
const BUYER = { accountId: '123456789012', licenseArn: 'arn:aws:license:test', productCode: 'p' };
const CONFIG = { productCode: 'p', region: 'us-east-1', origin: 'https://oya.example' };
const NOW = 1_800_000_000_000;
const MFA_TOKEN = `header.${Buffer.from(JSON.stringify({ aal: 'aal1' })).toString('base64url')}.signature`;
let connect: ReturnType<typeof mock.fn>;
let resolve: ReturnType<typeof mock.fn>;
let service: MarketplaceService;

beforeEach(() => {
  connect = mock.fn(async () => {});
  resolve = mock.fn(async () => BUYER);
  service = new MarketplaceService({
    config: () => CONFIG,
    resolve,
    now: () => NOW,
    repository: { connect, registered: async () => false },
  });
  mock.method(sessionUsers, 'configured', () => true);
  mock.method(sessionUsers, 'getUser', async (token: string) => ({
    data: {
      user:
        token === 'account' || token === MFA_TOKEN
          ? { id: 'alice', factors: token === MFA_TOKEN ? [{ status: 'verified' }] : [] }
          : null,
    },
    error: null,
  }));
});
afterEach(() => mock.restoreAll());

/** Runs the real body parser with a stream; records redirect status without opening a socket. */
function callback(form: string) {
  const req = Object.assign(Readable.from([Buffer.from(form)]), {
    method: 'POST',
    url: '/',
    originalUrl: '/marketplace/aws',
    ip: '127.0.0.1',
    headers: { 'content-type': 'application/x-www-form-urlencoded', 'content-length': String(Buffer.byteLength(form)) },
  });
  const res = new FakeResponse();
  return new Promise<FakeResponse>((done, reject) => {
    res.redirect = ((status: number, url: string) => {
      res.status(status);
      res.location = url;
      done(res);
      return res;
    }) as any;
    marketplaceCallback(service, () => CONFIG)(req as any, res as any, reject);
  });
}

/** Constructs a request carrying only an authenticated account token and sealed server context. */
async function apiRequest(path: string, token = 'account', extra = {}) {
  const value = await service.begin('aws-token');
  const cookie = encodeURIComponent(value);
  return fakeRequest({
    method: path.endsWith('connect') ? 'POST' : 'GET',
    path,
    body: {},
    headers: {
      authorization: token ? `Bearer ${token}` : '',
      cookie: `oya_aws_setup=${cookie}`,
      origin: CONFIG.origin,
      'idempotency-key': 'stable-request-key-for-tests',
      'x-oya-marketplace-context': service.context(value).contextId,
      ...extra,
    },
  });
}

describe('AWS fulfillment callback', () => {
  it('parses the real AWS form field, immediately resolves it and redirects with a clean 303', async () => {
    const res = await callback('x-amzn-marketplace-token=opaque%2Btoken&x-amzn-marketplace-offer-type=free-trial');
    assert.deepEqual(resolve.mock.calls[0].arguments, ['opaque+token', 'us-east-1']);
    assert.deepEqual([res.statusCode, res.location], [303, '/marketplace/aws']);
    assert.equal(res.cookies.oya_aws_setup.options.httpOnly, true);
    assert.equal(res.cookies.oya_aws_setup.options.secure, true);
    assert.equal(res.cookies.oya_aws_setup.options.sameSite, 'lax');
    assert.equal(res.cookies.oya_aws_pending.value, '1');
    assert.equal(openContext(res.cookies.oya_aws_setup.value, 'p', NOW).accountId, BUYER.accountId);
    assert.ok(!JSON.stringify(res).includes('opaque+token'));
    assert.equal(res.headers['Cache-Control'], 'no-store');
    assert.equal(connect.mock.callCount(), 0);
  });

  it('rejects missing, duplicate, oversized and unknown form data without calling AWS', async () => {
    for (const form of [
      '',
      'x-amzn-marketplace-token=a&x-amzn-marketplace-token=b',
      'x-amzn-marketplace-token=a&buyer=forged',
      `x-amzn-marketplace-token=${'a'.repeat(17_000)}`,
    ]) {
      const res = await callback(form);
      assert.deepEqual([res.statusCode, res.location], [303, '/marketplace/aws?setup=invalid']);
      assert.ok(res.cleared.oya_aws_setup);
    }
    assert.equal(resolve.mock.callCount(), 0);
  });

  it('removes stale context and hides provider diagnostics when AWS fails', async () => {
    resolve.mock.mockImplementation(async () => {
      throw new Error('raw AWS token secret');
    });
    const res = await callback('x-amzn-marketplace-token=test');
    assert.ok(res.cleared.oya_aws_setup);
    assert.ok(!JSON.stringify(res).includes('raw AWS token secret'));
    assert.equal(res.cookies.oya_aws_setup, undefined);
  });
});

describe('AWS account connection routes', () => {
  it('never connects on an anonymous or authenticated status read', async () => {
    const router = marketplaceRoutes(service, () => CONFIG);
    for (const path of ['/marketplace/aws', '/marketplace/aws/status']) {
      const res = await routeThrough(router, await apiRequest(path));
      assert.equal(res.body.status, 'ready');
      assert.equal(res.body.awsAccount, '9012');
      assert.equal(typeof res.body.contextId, 'string');
    }
    assert.equal(connect.mock.callCount(), 0);
  });

  it('refuses missing tokens, project credentials and incomplete MFA before linking', async () => {
    const router = marketplaceRoutes(service, () => CONFIG);
    for (const token of ['', 'project-key', MFA_TOKEN]) {
      const res = await routeThrough(router, await apiRequest('/marketplace/aws/connect', token));
      assert.equal(res.statusCode, 401);
    }
    assert.equal(connect.mock.callCount(), 0);
  });

  it('requires same-origin consent and forbids client identity fields or missing idempotency headers', async () => {
    const router = marketplaceRoutes(service, () => CONFIG);
    const cross = await routeThrough(
      router,
      await apiRequest('/marketplace/aws/connect', 'account', { origin: 'https://attacker.example' }),
    );
    assert.equal(cross.statusCode, 403);
    const forged = await apiRequest('/marketplace/aws/connect');
    forged.body = { user_id: 'bob' };
    assert.equal((await routeThrough(router, forged)).statusCode, 400);
    const absent = await routeThrough(
      router,
      await apiRequest('/marketplace/aws/connect', 'account', { 'idempotency-key': '' }),
    );
    assert.equal(absent.statusCode, 400);
    assert.equal(connect.mock.callCount(), 0);
    assert.equal((await routeThrough(router, await apiRequest('/marketplace/aws/connect'))).statusCode, 200);
    assert.deepEqual(connect.mock.calls[0].arguments, [BUYER, 'alice', NOW]);
  });

  it('clears expired context and its navigation hint', async () => {
    const router = marketplaceRoutes(service, () => CONFIG);
    const req = fakeRequest({ path: '/marketplace/aws', headers: { cookie: 'oya_aws_setup=tampered' } });
    const res = await routeThrough(router, req);
    assert.equal(res.statusCode, 400);
    assert.ok(res.cleared.oya_aws_setup);
    assert.ok(res.cleared.oya_aws_pending);
  });

  it('refuses consent for an earlier tab when a new handoff replaces the shared cookie', async () => {
    const router = marketplaceRoutes(service, () => CONFIG);
    const firstTab = await apiRequest('/marketplace/aws/connect');
    const secondBuyer = { ...BUYER, accountId: '987654323456', licenseArn: 'arn:aws:license:second' };
    resolve.mock.mockImplementation(async () => secondBuyer);
    const secondTab = await apiRequest('/marketplace/aws/connect');
    firstTab.headers.cookie = secondTab.headers.cookie;
    const refused = await routeThrough(router, firstTab);
    assert.deepEqual([refused.statusCode, refused.body.code], [409, 'setup_changed']);
    assert.equal(connect.mock.callCount(), 0);
    assert.equal((await routeThrough(router, secondTab)).statusCode, 200);
    assert.deepEqual(connect.mock.calls[0].arguments, [secondBuyer, 'alice', NOW]);
  });
});
