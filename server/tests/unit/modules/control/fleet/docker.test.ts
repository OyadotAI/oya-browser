/**
 * Unit tests for the Docker fleet runtime, against a fake docker CLI on PATH:
 * an internal bridge network and a governance-labelled image are required,
 * containers are hardened with credentials passed only through an env file,
 * and removal happens only on the same daemon for the container's own owner.
 */
import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { ownDataDir, restoreEnv } from '../../../support/data-dir.ts';

const dir = ownDataDir('oya-control-fleet-docker-');
const { control, hash } = await import('../../../../../src/modules/control/service.ts');
const docker = await import('../../../../../src/modules/control/fleet/docker.ts');
const { cliCalls, fakeCli } = await import('../../../support/control.ts');

/** The fake docker: answers from FAKE_* variables, and copies the env file it is given. */
const DOCKER = `(args) => {
  const e = process.env, a = args.join(' ');
  if (a.startsWith('network inspect')) return { stdout: e.FAKE_NETWORK };
  if (a.startsWith('image inspect')) return { stdout: e.FAKE_IMAGE };
  if (a.startsWith('info')) return { stdout: 'daemon-1\\n' };
  if (args[0] === 'create') {
    const file = args[args.indexOf('--env-file') + 1];
    require('node:fs').writeFileSync(e.FAKE_ENV_COPY, require('node:fs').readFileSync(file));
    e.FAKE_ENV_FILE && require('node:fs').writeFileSync(e.FAKE_ENV_FILE, file);
    return {};
  }
  if (args[0] === 'inspect') return e.FAKE_CONTAINER ? { stdout: e.FAKE_CONTAINER } : { stderr: 'Error: No such object: x', code: 1 };
  return {};
}`;
const ENV = [
  'OYA_MANAGED_NETWORK',
  'OYA_MANAGED_IMAGE',
  'OYA_MANAGED_CONTROL_URL',
  'OYA_MANAGED_PROXY_URL',
  'FAKE_NETWORK',
  'FAKE_IMAGE',
  'FAKE_CONTAINER',
  'FAKE_ENV_COPY',
  'FAKE_ENV_FILE',
];

let saved, fake;
beforeEach(() => {
  saved = Object.fromEntries(ENV.map((name) => [name, process.env[name]]));
  Object.assign(process.env, {
    OYA_MANAGED_NETWORK: 'browsers',
    OYA_MANAGED_IMAGE: 'oya-browser',
    OYA_MANAGED_CONTROL_URL: 'http://control',
    OYA_MANAGED_PROXY_URL: 'http://egress:3128',
    FAKE_NETWORK: JSON.stringify([{ Id: 'net-1', Internal: true, Driver: 'bridge' }]),
    FAKE_IMAGE: JSON.stringify([{ Id: 'img-1', Config: { Labels: { 'ai.getoya.governance': '1' } } }]),
    FAKE_ENV_COPY: join(dir, 'env-copy'),
    FAKE_ENV_FILE: join(dir, 'env-path'),
  });
  fake = fakeCli('docker', DOCKER, { readStdin: false });
});
afterEach(() => {
  fake.restore();
  for (const name of ENV) restoreEnv(name, saved[name]);
});

describe('docker verifyRuntime', () => {
  it('returns the daemon, network and image sessions are pinned to', async () => {
    const identity = await docker.verifyRuntime();
    assert.deepEqual(
      [identity.id, identity.daemonId, identity.networkId, identity.imageId, identity.region],
      ['docker-local', 'daemon-1', 'net-1', 'img-1', 'local'],
    );
  });

  it('refuses an unconfigured runtime', async () => {
    delete process.env.OYA_MANAGED_IMAGE;
    assert.equal(docker.configured(), false);
    await assert.rejects(docker.verifyRuntime(), { status: 422, code: 'runtime_unavailable' });
  });

  it('refuses a network that is not an internal bridge', async () => {
    process.env.FAKE_NETWORK = JSON.stringify([{ Internal: false, Driver: 'bridge' }]);
    await assert.rejects(docker.verifyRuntime(), { code: 'unsafe_network' });
  });

  it('refuses an image without the governance label', async () => {
    process.env.FAKE_IMAGE = JSON.stringify([{ Config: { Labels: {} } }]);
    await assert.rejects(docker.verifyRuntime(), { code: 'unsupported_image' });
  });
});

describe('docker create', () => {
  it('starts a hardened, labelled container with credentials only in a removed env file', async () => {
    await control().reserve('key-a', { id: 'db1', provider: 'oya-selfhosted', managed: true });
    await docker.create({ apiKey: 'key-a', browserId: 'db1', persona: 'p', name: 'n' });
    const calls = await cliCalls(fake.log);
    const create = calls.find((c) => c.args[0] === 'create').args;
    for (const flag of ['--cap-drop', 'ALL', 'no-new-privileges', '--pids-limit', '--network', 'net-1'])
      assert.ok(create.includes(flag), flag);
    assert.ok(create.includes(`oya.owner=${hash('key-a')}`));
    assert.ok(create.includes('oya.session=db1'));
    assert.equal(create.join(' ').includes('oya_'), false, 'no credential in argv');
    assert.match(readFileSync(process.env.FAKE_ENV_COPY, 'utf8'), /^OYA_API_KEY=oya_/m);
    assert.equal(existsSync(readFileSync(process.env.FAKE_ENV_FILE, 'utf8')), false, 'env file removed');
    assert.deepEqual(calls.at(-1).args, ['start', 'oya-managed-db1']);
    assert.equal((await control().store.get('session', 'db1')).cleanup.daemonId, 'daemon-1');
  });
});

describe('docker remove', () => {
  /** A container labelled for `apiKey` and session b1. */
  const container = (apiKey = 'key-a') =>
    JSON.stringify([{ Config: { Labels: { 'oya.owner': hash(apiKey), 'oya.session': 'b1' } } }]);

  it('force-removes the owner’s container', async () => {
    process.env.FAKE_CONTAINER = container();
    await docker.remove('c1', 'key-a', 'b1', 'daemon-1');
    assert.deepEqual((await cliCalls(fake.log)).at(-1).args, ['rm', '-f', 'c1']);
  });

  it('refuses to clean up on another daemon', async () => {
    await assert.rejects(docker.remove('c1', 'key-a', 'b1', 'daemon-2'), { status: 503, code: 'runtime_unavailable' });
  });

  it('refuses a container another owner made', async () => {
    process.env.FAKE_CONTAINER = container('key-b');
    await assert.rejects(docker.remove('c1', 'key-a', 'b1'), { code: 'ownership_mismatch' });
  });

  it('treats a container that no longer exists as removed', async () => {
    await docker.remove('c1', 'key-a', 'b1');
    assert.equal(
      (await cliCalls(fake.log)).some((c) => c.args[0] === 'rm'),
      false,
    );
  });
});
