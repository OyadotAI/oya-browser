/**
 * Unit tests for the Docker sandbox runtime against a fake docker CLI:
 * the container it runs, that the API key reaches it only through the env
 * file, and finding, deleting and listing containers by their labels.
 */
import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { fakeCli } from '../../../support/fake-cli.ts';
import * as docker from '../../../../../src/drivers/sandbox/workers/docker.ts';

/** Labels as the fake's containers carry them, set per test. */
const cli = fakeCli(
  'docker',
  `
if (args[0] === 'run') reply('cid-123\\n');
if (args[0] === 'ps') reply(process.env.FAKE_PS || '');
if (args[0] === 'rm') reply('');
if (args[0] === 'inspect') {
  if (args.includes('missing')) fail('Error: No such object: missing');
  if (args.includes('broken')) fail('Cannot connect to the Docker daemon');
  reply(args.slice(1).map((id) => ({ Id: id, Config: { Labels: { 'oya-browser-id': id } }, State: { Status: 'running' } })));
}
fail('unhandled ' + args.join(' '));
`,
);

/** A configured deployment. */
const CONFIG = { image: 'oya/browser:1', network: 'oya-net' };
/** What the facade hands every runtime. */
const SPEC = {
  name: 'oya-browser-b-1',
  labels: { 'oya-browser': 'true', 'oya-owner': 'owner-tag', 'oya-name': 'My browser' },
  env: { OYA_API_KEY: 'secret-key', OYA_BROWSER_ID: 'b-1' },
  ttlMinutes: 60,
  lifetimeMinutes: 70,
};

beforeEach(() => cli.reset());

describe('docker settings', () => {
  it('needs only the image, the network being optional', () => {
    assert.equal(docker.settings({}), null);
    assert.deepEqual(docker.missing({}), ['OYA_CLOUD_IMAGE']);
    assert.deepEqual(docker.settings({ OYA_CLOUD_IMAGE: 'i' }), { image: 'i', network: null, platform: null });
  });

  it('is never configured by a key', () => {
    assert.equal(docker.ownAccount(), false);
  });
});

describe('docker create', () => {
  it('runs a hardened, labelled, self-removing container of the image', async () => {
    assert.deepEqual(await docker.create(CONFIG, SPEC), { id: 'cid-123' });
    const [{ args }] = cli.calls();
    assert.deepEqual(args.slice(0, 5), ['run', '-d', '--rm', '--name', 'oya-browser-b-1']);
    for (const flag of ['--cap-drop', '--security-opt', '--pids-limit', '--memory'])
      assert.ok(args.includes(flag), flag);
    assert.ok(args.includes('oya-name=My browser'));
    assert.deepEqual(args.slice(args.indexOf('--network'), args.indexOf('--network') + 2), ['--network', 'oya-net']);
    assert.equal(args.at(-1), 'oya/browser:1');
  });

  it('asks for a platform only when one is set, for an image of another architecture', async () => {
    await docker.create(CONFIG, SPEC);
    assert.ok(!cli.calls()[0].args.includes('--platform'));
    cli.reset();
    await docker.create({ ...CONFIG, platform: 'linux/amd64' }, SPEC);
    const { args } = cli.calls()[0];
    assert.deepEqual(args.slice(args.indexOf('--platform'), args.indexOf('--platform') + 2), [
      '--platform',
      'linux/amd64',
    ]);
  });

  it('passes the API key in the env file, never in argv', async () => {
    await docker.create(CONFIG, SPEC);
    const [{ args, files }] = cli.calls();
    assert.ok(!args.join(' ').includes('secret-key'));
    assert.match(Object.values(files)[0], /^OYA_API_KEY=secret-key$/m);
  });
});

describe('docker find, destroy and list', () => {
  it('finds a container by name and deletes it', async () => {
    const found = await docker.find(CONFIG, 'b-1');
    assert.deepEqual([found!.labels, found!.state], [{ 'oya-browser-id': 'b-1' }, 'running']);
    await found!.destroy();
    assert.deepEqual(cli.calls().at(-1)!.args, ['rm', '-f', 'b-1']);
  });

  it('answers null for a container the daemon does not have', async () => {
    assert.equal(await docker.find(CONFIG, 'missing'), null);
  });

  it('passes on any other daemon failure', async () => {
    await assert.rejects(docker.find(CONFIG, 'broken'), /Cannot connect/);
  });

  it('lists the owner’s containers by label, and asks nothing more when there are none', async () => {
    process.env.FAKE_PS = 'c1\nc2\n';
    const rows = await docker.list(CONFIG, 'owner-tag');
    assert.deepEqual(
      rows.map((row) => row.labels['oya-browser-id']),
      ['c1', 'c2'],
    );
    assert.ok(cli.calls()[0].args.includes('label=oya-owner=owner-tag'));
    process.env.FAKE_PS = '';
    cli.reset();
    assert.deepEqual(await docker.list(CONFIG, 'owner-tag'), []);
    assert.equal(cli.calls().length, 1);
    delete process.env.FAKE_PS;
  });
});
