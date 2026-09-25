/**
 * Unit tests for the helpers the REST routes share: the caller's key, the
 * operator gate, per-key browser access, the SDK file and data checks, the
 * webhook announcement and the long-running JSON answer.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import {
  announce,
  apiKeyHeader,
  canAccess,
  evaluateIn,
  getKey,
  longJson,
  MAX_FILE_BYTES,
  operatorOnly,
  ownerScope,
  requireBrowser,
  validData,
  validFile,
} from '../../../src/app/http.ts';
import { fingerprint } from '../../../src/platform/audit.ts';
import { HttpError } from '../../../src/platform/errors.ts';
import { LONG_JSON_KEEPALIVE_MS } from '../../../src/app/constants.ts';
import { connectBrowser, disconnectBrowser } from '../support/fakes.ts';
import { FakeResponse, driveBrowser, fakeRequest, stubControl } from '../support/browsers.ts';

const B = 'b-http';

describe('an X-API-Key header', () => {
  /** Runs the middleware over these headers and returns them. */
  const through = (headers) => {
    const next = mock.fn();
    apiKeyHeader({ headers }, null, next);
    assert.equal(next.mock.callCount(), 1);
    return headers;
  };

  it('becomes the bearer key, so directories that cannot add "Bearer " still authenticate', () => {
    assert.equal(getKey({ headers: through({ 'x-api-key': 'k1' }) }), 'k1');
  });

  it('never overrides an Authorization header', () => {
    assert.equal(through({ 'x-api-key': 'k1', authorization: 'Bearer k2' }).authorization, 'Bearer k2');
  });

  it('adds nothing when it is absent or empty', () => {
    assert.equal(through({ 'x-api-key': '' }).authorization, undefined);
  });
});

describe('the caller’s key', () => {
  it('is the bearer token, or empty without one', () => {
    assert.equal(getKey(fakeRequest({ key: 'k1' })), 'k1');
    assert.equal(getKey(fakeRequest({ key: '' })), '');
  });

  it('scopes everything to the key’s fingerprint', () => {
    assert.equal(ownerScope(fakeRequest({ key: 'k1' })), fingerprint('k1'));
  });
});

describe('operatorOnly', () => {
  const saved = { op: process.env.OYA_OPERATOR_TOKEN, metrics: process.env.OYA_METRICS_TOKEN };
  afterEach(() => {
    for (const [name, value] of [
      ['OYA_OPERATOR_TOKEN', saved.op],
      ['OYA_METRICS_TOKEN', saved.metrics],
    ])
      if (value === undefined) delete process.env[name];
      else process.env[name] = value;
  });

  /** Runs the gate for `key`; returns whether it passed and the response. */
  function gate(key: string) {
    const res = new FakeResponse();
    const next = mock.fn();
    operatorOnly(fakeRequest({ key }), res, next);
    return { passed: next.mock.callCount() === 1, res };
  }

  it('lets the operator token through', () => {
    process.env.OYA_OPERATOR_TOKEN = 'op-secret';
    assert.equal(gate('op-secret').passed, true);
  });

  it('accepts the metrics token when no operator token is set', () => {
    delete process.env.OYA_OPERATOR_TOKEN;
    process.env.OYA_METRICS_TOKEN = 'm-secret';
    assert.equal(gate('m-secret').passed, true);
  });

  it('refuses any other key with 403', () => {
    process.env.OYA_OPERATOR_TOKEN = 'op-secret';
    const { passed, res } = gate('op-secreT');
    assert.equal(passed, false);
    assert.equal(res.statusCode, 403);
  });

  it('refuses everyone when no token is configured', () => {
    delete process.env.OYA_OPERATOR_TOKEN;
    delete process.env.OYA_METRICS_TOKEN;
    assert.equal(gate('').passed, false);
    assert.equal(gate('anything').passed, false);
  });
});

describe('browser access', () => {
  afterEach(() => disconnectBrowser(B));

  it('reaches a browser only with the key that connected it', () => {
    connectBrowser(B, 'k1');
    assert.equal(canAccess(fakeRequest({ key: 'k1' }), B), true);
    assert.equal(canAccess(fakeRequest({ key: 'k2' }), B), false);
  });

  it('passes a route on for the owner’s connected browser', () => {
    connectBrowser(B, 'k1');
    const next = mock.fn();
    requireBrowser(fakeRequest({ key: 'k1', params: { browserId: B } }), new FakeResponse(), next);
    assert.equal(next.mock.callCount(), 1);
  });

  it('answers 404, not 403, for another key’s browser', () => {
    connectBrowser(B, 'k1');
    const res = new FakeResponse();
    const next = mock.fn();
    requireBrowser(fakeRequest({ key: 'k2', params: { browserId: B } }), res, next);
    assert.deepEqual(
      [res.statusCode, res.body, next.mock.callCount()],
      [404, { error: `Browser ${B} not connected` }, 0],
    );
  });
});

describe('evaluateIn', () => {
  afterEach(() => {
    mock.restoreAll();
    disconnectBrowser(B);
  });

  it('runs the script as evaluate_raw and unwraps its result', async () => {
    stubControl();
    const driver = driveBrowser(B, () => ({ ok: true, data: { result: 42 } }));
    assert.equal(await evaluateIn(B, '6*7'), 42);
    assert.deepEqual(driver.sent[0], {
      action: 'evaluate_raw',
      params: { expression: '6*7' },
      timeout: driver.sent[0].timeout,
    });
  });

  it('falls back to the data, then null', async () => {
    stubControl();
    driveBrowser(B, (_a, p) => (p.expression === 'a' ? { ok: true, data: 'raw' } : { ok: true }));
    assert.equal(await evaluateIn(B, 'a'), 'raw');
    assert.equal(await evaluateIn(B, 'b'), null);
  });
});

describe('validFile and validData', () => {
  const file = { file: 'a.txt', type: 'text/plain', b64: 'YWJj' };

  it('accepts a well-formed file', () => {
    assert.equal(validFile(file), true);
  });

  it('refuses a file with a bad name, type, body or size', () => {
    assert.equal(validFile(null), false);
    assert.equal(validFile([file]), false);
    assert.equal(validFile({ ...file, file: '' }), false);
    assert.equal(validFile({ ...file, type: 't'.repeat(200) }), false);
    assert.equal(validFile({ ...file, b64: 'not base64!' }), false);
    assert.equal(validFile({ ...file, b64: 'A'.repeat(Math.ceil(MAX_FILE_BYTES / 3) * 4 + 4) }), false);
  });

  it('maps word-character names to strings, numbers or files', () => {
    assert.equal(validData({ name: 'ada', age: 36, cv: file }), true);
    assert.equal(validData({}), true);
  });

  it('refuses bad names, nested values, arrays and non-objects', () => {
    assert.equal(validData({ 'has space': 'x' }), false);
    assert.equal(validData({ ['n'.repeat(65)]: 'x' }), false);
    assert.equal(validData({ nested: { a: 1 } }), false);
    assert.equal(validData(['x']), false);
    assert.equal(validData('x'), false);
  });

  it('refuses files where files are not allowed, as for secrets', () => {
    assert.equal(validData({ cv: file }, { files: false }), false);
  });
});

describe('announce', () => {
  afterEach(() => mock.restoreAll());

  it('emits a control-plane event for the key', async () => {
    const { emit } = stubControl();
    await announce('k1', 'browser.ready', 's1', { a: 1 });
    assert.deepEqual(emit.mock.calls[0].arguments, ['k1', 'browser.ready', 's1', { a: 1 }]);
  });

  it('never fails the caller when the event cannot be sent', async () => {
    stubControl({ emit: async () => Promise.reject(new Error('down')) });
    await assert.doesNotReject(announce('k1', 't', null, {}));
  });
});

describe('longJson', () => {
  afterEach(() => {
    mock.timers.reset();
    mock.restoreAll();
  });

  it('commits to 200 and ends with the work’s result', async () => {
    const res = new FakeResponse();
    await longJson(res, async () => ({ ok: 1 }));
    assert.equal(res.statusCode, 200);
    assert.equal(res.headers['Content-Type'], 'application/json');
    assert.deepEqual(JSON.parse(res.ended), { ok: 1 });
  });

  it('trickles whitespace while the work runs, then stops', async () => {
    mock.timers.enable({ apis: ['setInterval'] });
    const res = new FakeResponse();
    let finish;
    const done = longJson(res, () => new Promise((r) => (finish = r)));
    mock.timers.tick(LONG_JSON_KEEPALIVE_MS * 2);
    finish('x');
    await done;
    mock.timers.tick(LONG_JSON_KEEPALIVE_MS * 2);
    assert.deepEqual(res.written, [' ', ' ']);
  });

  it('carries an HttpError’s real status, code and message in the body once the 200 is out', async () => {
    const res = new FakeResponse();
    await longJson(res, async () => {
      throw new HttpError(429, 'quota', { code: 'rate_limited' });
    });
    assert.deepEqual(JSON.parse(res.ended), { error: 'quota', code: 'rate_limited', status: 429 });
  });

  it('hides an unexpected failure behind a 500 body with a reference, the same as any route', async () => {
    mock.method(console, 'error', () => {});
    const res = new FakeResponse();
    await longJson(res, async () => {
      throw new Error('boom');
    });
    const body = JSON.parse(res.ended);
    assert.deepEqual([body.status, body.code], [500, 'internal_error']);
    assert.ok(!body.error.includes('boom'));
    assert.match(body.ref, /^[0-9a-f]{8}$/);
  });
});
