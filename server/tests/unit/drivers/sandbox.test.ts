/**
 * Unit tests for Oya Cloud sandboxes with the runtime SDK client's calls
 * stubbed: creating a browser's sandbox, deleting only one this key owns, and
 * listing a key's sandboxes alongside its connected browsers.
 */
import { describe, it, before, beforeEach, afterEach, after, mock } from 'node:test';
import assert from 'node:assert/strict';
import { Status } from '../../../src/platform/http-status.ts';
import { ownDataDir, restoreEnv } from '../support/data-dir.ts';

ownDataDir('oya-sandbox-');
/** The settings a configured deployment has. */
const CLOUD = {
  OYA_CLOUD_API_KEY: 'cloud-key',
  OYA_CLOUD_SNAPSHOT: 'oya-browser:1',
  OYA_PUBLIC_WS_URL: 'wss://oya.example/ws',
};
const saved = Object.fromEntries(Object.keys(CLOUD).map((name) => [name, process.env[name]]));
Object.assign(process.env, CLOUD);
after(() => Object.entries(saved).forEach(([name, value]) => restoreEnv(name, value)));

const sandbox = await import('../../../src/drivers/sandbox.ts');
const { client } = await import('../../../src/drivers/sandbox/client.ts');
const { ownerTag, PREFIX } = await import('../../../src/drivers/sandbox/config.ts');
const { control } = await import('../../../src/modules/control/service.ts');

/** A sandbox as the SDK returns one, with the entrypoint already running unless told otherwise. */
function fakeSandbox({ labels = {}, image = 'ok', running = true }: any = {}) {
  return {
    id: 'sb-1',
    labels,
    setTtl: mock.fn(async () => {}),
    delete: mock.fn(async () => {}),
    process: {
      executeCommand: mock.fn(async () => ({ result: image })),
      getEntrypointSession: async () => ({
        commands: running ? [{ command: 'cd /app && /docker-entrypoint.sh', exitCode: null }] : [],
      }),
      createSession: mock.fn(async () => {}),
      executeSessionCommand: mock.fn(async () => {}),
    },
  };
}

let daytona: any;
before(async () => {
  daytona = await client();
});
afterEach(() => mock.restoreAll());

describe('createSandbox', () => {
  it('reserves a browser and creates its labelled sandbox with the enrolment contract', async () => {
    const box = fakeSandbox();
    const create = mock.method(daytona, 'create', async () => box);
    const made = await sandbox.createSandbox({ apiKey: 'user-key', name: 'Research', persona: 'p-1' });
    const [spec, options] = create.mock.calls[0].arguments as any[];
    assert.equal(made.sandboxName, PREFIX + made.browserId);
    assert.equal(spec.name, made.sandboxName);
    assert.deepEqual(spec.labels, {
      'oya-browser': 'true',
      'oya-browser-id': made.browserId,
      'oya-owner': ownerTag('user-key'),
      'oya-name': 'Research',
      'oya-persona': 'p-1',
    });
    assert.equal(spec.envVars.OYA_API_KEY, 'user-key');
    assert.equal(spec.envVars.OYA_SERVER_URL, 'wss://oya.example/ws');
    assert.equal(spec.envVars.OYA_PERSONA, 'p-1');
    assert.equal(spec.envVars.OYA_REMOTE_DEBUGGING_HOST, '127.0.0.1', 'the CDP port never leaves the sandbox');
    assert.ok(!JSON.stringify(spec.labels).includes('user-key'), 'labels carry a digest of the key, never the key');
    assert.ok(options.timeout > 0);
    assert.equal(box.setTtl.mock.callCount(), 1);
    assert.equal(sandbox.isProvisioned(made.browserId), true);
    const session = await control().findSession('user-key', made.browserId);
    assert.equal(session.provider, 'oya-cloud');
  });

  it('starts the browser in a legacy image whose entrypoint is not running', async () => {
    const box = fakeSandbox({ running: false });
    mock.method(daytona, 'create', async () => box);
    await sandbox.createSandbox({ apiKey: 'user-key' });
    assert.equal(box.process.createSession.mock.callCount(), 1);
    assert.match((box.process.executeSessionCommand.mock.calls[0].arguments as any[])[1].command, /docker-entrypoint/);
  });

  it('deletes a sandbox whose snapshot is not an Oya browser image, and says so', async () => {
    const box = fakeSandbox({ image: '' });
    mock.method(daytona, 'create', async () => box);
    await assert.rejects(sandbox.createSandbox({ apiKey: 'user-key' }), {
      status: Status.CONFLICT,
      message: /OYA_CLOUD_SNAPSHOT "oya-browser:1" has no \/docker-entrypoint\.sh/,
    });
    assert.equal(box.delete.mock.callCount(), 1);
  });

  it('requires an API key', async () => {
    await assert.rejects(sandbox.createSandbox({}), { status: Status.BAD_REQUEST, message: 'An API key is required' });
  });
});

describe('removeSandbox', () => {
  it('deletes the sandbox of a browser this key owns', async () => {
    const box = fakeSandbox({ labels: { 'oya-browser-id': 'b-1', 'oya-owner': ownerTag('user-key') } });
    const get = mock.method(daytona, 'get', async () => box);
    assert.equal(await sandbox.removeSandbox('b-1', 'user-key'), true);
    assert.equal((get.mock.calls[0].arguments as any[])[0], `${PREFIX}b-1`);
    assert.equal(box.delete.mock.callCount(), 1);
    assert.equal(sandbox.isProvisioned('b-1'), false);
  });

  it('answers another key as if there were no sandbox, deleting nothing', async () => {
    const box = fakeSandbox({ labels: { 'oya-browser-id': 'b-1', 'oya-owner': ownerTag('user-key') } });
    mock.method(daytona, 'get', async () => box);
    assert.equal(await sandbox.removeSandbox('b-1', 'someone-else'), false);
    assert.equal(box.delete.mock.callCount(), 0);
  });

  it('answers false for a sandbox that does not exist', async () => {
    mock.method(daytona, 'get', async () => Promise.reject(Object.assign(new Error('nope'), { statusCode: 404 })));
    assert.equal(await sandbox.removeSandbox('b-404', 'user-key'), false);
  });

  it('refuses a sandbox whose labels do not name this browser', async () => {
    mock.method(daytona, 'get', async () => fakeSandbox({ labels: { 'oya-browser-id': 'b-other' } }));
    await assert.rejects(sandbox.removeSandbox('b-1', 'user-key'), { message: 'Sandbox ownership mismatch' });
  });

  it('requires an API key', async () => {
    await assert.rejects(sandbox.removeSandbox('b-1', ''), { status: Status.BAD_REQUEST });
  });
});

describe('listSandboxBrowsers', () => {
  /** The SDK's listing: an async iterable of these sandboxes. */
  const listing = (...boxes: any[]) =>
    mock.method(daytona, 'list', async function* () {
      yield* boxes;
    });
  /** Labels of a live browser sandbox owned by `key`. */
  const owned = (key: string, id: string, extra = {}) => ({
    'oya-browser': 'true',
    'oya-owner': ownerTag(key),
    'oya-browser-id': id,
    ...extra,
  });

  beforeEach(() => mock.timers.enable({ apis: ['Date'], now: Date.now() + 60_000 }));
  afterEach(() => mock.timers.reset());

  it('lists the key’s disconnected sandboxes as dead rows and overlays connected browsers', async () => {
    listing(
      { state: 'started', labels: owned('list-key', 'b-1', { 'oya-name': 'One', 'oya-persona': 'p-9' }) },
      { state: 'stopped', labels: owned('list-key', 'b-2') },
      { state: 'started', labels: owned('other-key', 'b-3') },
      { state: 'deleted', labels: owned('list-key', 'b-4') },
      { state: 'started', labels: { ...owned('list-key', 'b-5'), 'oya-browser': 'false' } },
    );
    const rows = await sandbox.listSandboxBrowsers('list-key', [
      { id: 'b-2', health: 'ok' },
      { id: 'b-x', health: 'ok' },
    ]);
    const byId = Object.fromEntries(rows.map((r: any) => [r.id, r]));
    assert.deepEqual(Object.keys(byId).sort(), ['b-1', 'b-2', 'b-x'], 'never trusts the upstream filter');
    assert.deepEqual(
      [byId['b-1'].name, byId['b-1'].persona, byId['b-1'].health, byId['b-1'].sandboxState],
      ['One', 'p-9', 'dead', 'started'],
    );
    assert.deepEqual(byId['b-2'], { id: 'b-2', health: 'ok', provider: 'oya-cloud' });
    assert.deepEqual(byId['b-x'], { id: 'b-x', health: 'ok' });
  });

  it('reuses a fresh inventory instead of asking again', async () => {
    const list = listing({ state: 'started', labels: owned('cache-key', 'b-1') });
    await sandbox.listSandboxBrowsers('cache-key');
    await sandbox.listSandboxBrowsers('cache-key');
    assert.equal(list.mock.callCount(), 1);
  });

  it('answers with what it has when a refresh fails', async () => {
    mock.method(console, 'warn', () => {});
    mock.method(daytona, 'list', () => {
      throw new Error('runtime down');
    });
    assert.deepEqual(await sandbox.listSandboxBrowsers('failing-key', [{ id: 'c' }]), [{ id: 'c' }]);
  });

  it('passes connected browsers through without a key', async () => {
    assert.deepEqual(await sandbox.listSandboxBrowsers('', [{ id: 'c' }]), [{ id: 'c' }]);
  });
});
