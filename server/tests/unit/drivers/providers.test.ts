/**
 * Unit tests for where a CDP browser comes from: a direct CDP URL, or a hosted
 * vendor session created, read and released through a stubbed fetch, with the
 * sealed cleanup record another process can release from.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { Status } from '../../../src/platform/http-status.ts';
import { ownDataDir } from '../support/data-dir.ts';
import { json, stubFetch, text } from '../support/http.ts';

ownDataDir('oya-providers-');
const { acquire, available, releasePersisted } = await import('../../../src/drivers/providers.ts');

/** A deployment with a Steel key. */
const STEEL = { STEEL_API_KEY: 'steel-key' };

afterEach(() => mock.restoreAll());

describe('available', () => {
  it('always offers direct CDP, and each vendor as configured only with its key', () => {
    const listed = available(STEEL);
    assert.deepEqual(listed[0], {
      name: 'cdp',
      kind: 'direct',
      configured: true,
      note: 'Bring your own CDP WebSocket URL',
    });
    const byName = Object.fromEntries(listed.map((p) => [p.name, p]));
    assert.deepEqual(byName.steel, { name: 'steel', kind: 'hosted', configured: true, envVar: 'STEEL_API_KEY' });
    assert.equal(byName.anchor.configured, false);
    assert.ok(byName.browserbase && byName.browseruse);
  });
});

describe('acquire a direct CDP browser', () => {
  it('takes a ws:// or wss:// URL, with nothing to release', async () => {
    const got = await acquire({ wsUrl: 'ws://127.0.0.1:9222/devtools/browser/x' });
    assert.deepEqual([got.wsUrl, got.provider, got.sessionId], ['ws://127.0.0.1:9222/devtools/browser/x', 'cdp', null]);
    assert.equal(await got.release(), undefined);
  });

  it('refuses a missing, unparseable or unusable URL with 400', async () => {
    for (const [wsUrl, message] of [
      [undefined, 'wsUrl is required for the cdp provider'],
      ['::::', 'wsUrl is not a valid URL'],
      ['ftp://127.0.0.1:9222', 'wsUrl must be ws://, wss://, or the http address of a debugging port'],
    ]) {
      await assert.rejects(acquire({ wsUrl }), { status: Status.BAD_REQUEST, message });
    }
  });

  it('resolves the http address Chrome prints for a debugging port', async () => {
    const calls = stubFetch(() => json({ webSocketDebuggerUrl: 'ws://127.0.0.1:9222/devtools/browser/abc' }));
    const got = await acquire({ wsUrl: 'http://127.0.0.1:9222' });
    assert.equal(got.wsUrl, 'ws://127.0.0.1:9222/devtools/browser/abc');
    assert.equal(calls[0].url, 'http://127.0.0.1:9222/json/version');
  });

  it('refuses a debugging port that points somewhere else', async () => {
    stubFetch(() => json({ webSocketDebuggerUrl: 'ws://evil.test/devtools/browser/abc' }));
    await assert.rejects(acquire({ wsUrl: 'http://127.0.0.1:9222' }), {
      status: Status.BAD_REQUEST,
      message: /pointed at evil.test, not itself/,
    });
  });

  it('says so when nothing answers on the debugging port', async () => {
    stubFetch(() => text('not chrome', 404));
    await assert.rejects(acquire({ wsUrl: 'http://127.0.0.1:9222' }), {
      status: Status.BAD_REQUEST,
      message: /answered 404, so it is not a Chrome debugging port/,
    });
  });
});

describe('acquire a hosted vendor session', () => {
  it('creates a session and connects to its CDP URL, adding the key where the vendor wants it', async () => {
    const calls = stubFetch(() => json({ id: 'sess-1', websocketUrl: 'wss://connect.steel.dev/?x=1' }));
    const got = await acquire({ provider: 'steel', env: STEEL });
    assert.equal(got.wsUrl, 'wss://connect.steel.dev/?x=1&apiKey=steel-key');
    assert.equal(got.sessionId, 'sess-1');
    assert.deepEqual(
      [calls[0].url, calls[0].init.method, calls[0].init.redirect],
      ['https://api.steel.dev/v1/sessions', 'POST', 'error'],
    );
    assert.equal(calls[0].init.headers['steel-api-key'], 'steel-key');
  });

  it('reads fields where each vendor puts them', async () => {
    stubFetch(() => json({ data: { id: 'a-1', cdp_url: 'wss://anchor.example/cdp' } }));
    const got = await acquire({ provider: 'anchor', env: { ANCHOR_API_KEY: 'k' } });
    assert.deepEqual([got.wsUrl, got.sessionId], ['wss://anchor.example/cdp', 'a-1']);
  });

  it('sends the Browserbase project id when one is configured', async () => {
    const calls = stubFetch(() => json({ id: 'b', connectUrl: 'wss://bb.example' }));
    await acquire({ provider: 'browserbase', env: { BROWSERBASE_API_KEY: 'k', BROWSERBASE_PROJECT_ID: 'proj' } });
    assert.deepEqual(JSON.parse(calls[0].init.body), { projectId: 'proj' });
  });

  it('refuses an unknown vendor with 400 and an unconfigured one with 409', async () => {
    await assert.rejects(acquire({ provider: 'nope', env: {} }), {
      status: Status.BAD_REQUEST,
      message: 'Unknown browser provider: nope',
    });
    await assert.rejects(acquire({ provider: 'steel', env: {} }), {
      status: Status.CONFLICT,
      message: 'steel is not configured, set STEEL_API_KEY',
    });
  });

  it('answers 502 without echoing the vendor’s body when the create is refused', async () => {
    stubFetch(() => text('{"secret":"wss://leak"}', 402));
    await assert.rejects(acquire({ provider: 'steel', env: STEEL }), (err: any) => {
      assert.equal(err.status, Status.BAD_GATEWAY);
      assert.match(err.message, /steel session create failed \(402\)/);
      assert.doesNotMatch(err.message, /leak/);
      return true;
    });
  });

  it('answers 502 when the vendor cannot be reached', async () => {
    stubFetch(() => Promise.reject(new Error('ECONNREFUSED')));
    await assert.rejects(acquire({ provider: 'steel', env: STEEL }), {
      status: Status.BAD_GATEWAY,
      message: /create request failed/,
    });
  });

  it('releases the session rather than leaking it when there is no usable CDP URL', async () => {
    const calls = stubFetch((url) =>
      url.endsWith('/sessions') ? json({ id: 's-9', websocketUrl: 'https://x' }) : json({}),
    );
    await assert.rejects(acquire({ provider: 'steel', env: STEEL }), {
      status: Status.BAD_GATEWAY,
      message: /valid CDP URL/,
    });
    assert.deepEqual([calls[1].url, calls[1].init.method], ['https://api.steel.dev/v1/sessions/s-9/release', 'POST']);
  });

  it('adds a failed cleanup to the original error', async () => {
    stubFetch((url) => (url.endsWith('/sessions') ? json({ id: 's-9' }) : json({}, 500)));
    await assert.rejects(acquire({ provider: 'steel', env: STEEL }), {
      message: /valid CDP URL.*Cleanup also failed: steel session s-9 release failed \(500\)/,
    });
  });

  it('hands the sealed cleanup record to onCreated before connecting', async () => {
    stubFetch(() => json({ id: 's-1', websocketUrl: 'wss://x' }));
    const onCreated = mock.fn(async () => {});
    const got = await acquire({ provider: 'steel', env: STEEL, onCreated });
    const [record] = onCreated.mock.calls[0].arguments as any[];
    assert.equal(record.kind, 'vendor');
    assert.doesNotMatch(record.sealed, /steel-key/, 'the vendor key is sealed');
    assert.equal(got.cleanup, record);
  });

  it('releases a session once, retrying only after a failure, and counts a vanished one as released', async () => {
    const answers = [json({}, 500), json({}, Status.NOT_FOUND)];
    const calls = stubFetch((url) =>
      url.endsWith('/sessions') ? json({ id: 'bb-1', connectUrl: 'wss://x' }) : answers.shift()!,
    );
    const got = await acquire({ provider: 'browserbase', env: { BROWSERBASE_API_KEY: 'k' } });
    await assert.rejects(got.release(), {
      status: Status.BAD_GATEWAY,
      message: 'browserbase session bb-1 release failed (500)',
    });
    await got.release();
    await got.release();
    const releases = calls.slice(1);
    assert.equal(releases.length, 2);
    assert.deepEqual(
      [releases[0].url, JSON.parse(releases[0].init.body)],
      ['https://api.browserbase.com/v1/sessions/bb-1', { status: 'REQUEST_RELEASE' }],
    );
  });

  it('takes a vendor from OYA_BROWSER_PROVIDERS, with a {id} release template', async () => {
    const custom = {
      createUrl: 'https://vendor.example/new',
      headers: { 'x-key': 'fixed' },
      wsPath: ['ws'],
      idPath: ['sid'],
      deleteUrl: 'https://vendor.example/end/{id}',
    };
    const env = { OYA_BROWSER_PROVIDERS: JSON.stringify({ custom }), CUSTOM_API_KEY: 'k' };
    const calls = stubFetch((url) => (url.endsWith('/new') ? json({ sid: 'a/b', ws: 'wss://v.example' }) : json({})));
    const got = await acquire({ provider: 'custom', env });
    await got.release();
    assert.equal(calls[0].init.headers['x-key'], 'fixed');
    assert.deepEqual([calls[1].url, calls[1].init.method], ['https://vendor.example/end/a%2Fb', 'DELETE']);
  });
});

describe('releasePersisted', () => {
  it('replays the sealed release call, treating 410 as released', async () => {
    const calls = stubFetch((url) =>
      url.endsWith('/sessions') ? json({ id: 's-1', websocketUrl: 'wss://x' }) : json({}, 410),
    );
    const onCreated = mock.fn(async () => {});
    await acquire({ provider: 'steel', env: STEEL, onCreated });
    await releasePersisted((onCreated.mock.calls[0].arguments as any[])[0]);
    assert.deepEqual(
      [calls[1].url, calls[1].init.method, calls[1].init.headers['steel-api-key']],
      ['https://api.steel.dev/v1/sessions/s-1/release', 'POST', 'steel-key'],
    );
  });

  it('fails when the vendor refuses the release', async () => {
    stubFetch((url) => (url.endsWith('/sessions') ? json({ id: 's-1', websocketUrl: 'wss://x' }) : json({}, 500)));
    const onCreated = mock.fn(async () => {});
    await acquire({ provider: 'steel', env: STEEL, onCreated });
    await assert.rejects(releasePersisted((onCreated.mock.calls[0].arguments as any[])[0]), {
      message: 'Provider release failed (500)',
    });
  });
});
