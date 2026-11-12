/** Exercise readiness failure and smoke cleanup with fake HTTP tools and no browser or cloud access. */
import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync, spawn } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, readdirSync, existsSync, rmSync, copyFileSync, realpathSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, basename, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { once } from 'node:events';

const LIB = fileURLToPath(new URL('../deployments/lib.sh', import.meta.url));
const roots = [];

/** Minimal fake curl and jq preserve the script's public HTTP/JSON protocol. */
function fixture() {
  const root = mkdtempSync(join(tmpdir(), 'oya-smoke-')), bin = join(root, 'bin');
  roots.push(root); mkdirSync(bin);
  writeFileSync(join(bin, 'curl'), `#!${process.execPath}
const fs = require('node:fs'), args = process.argv.slice(2), url = args.at(-1);
fs.appendFileSync(process.env.OYA_TEST_ROOT + '/calls', url + '\\n');
if (!args.includes('--connect-timeout') || !args.includes('--max-time')) process.exit(8);
if (url.endsWith('/readyz')) process.exit(process.env.OYA_TEST_READY_FAIL ? 22 : 0);
if (url.endsWith('/start')) console.log(JSON.stringify({ id: 'fixture-browser' }));
else if (url.endsWith('/command')) console.log(JSON.stringify({ ok: !process.env.OYA_TEST_NAV_FAIL, data: { url: 'https://example.com' } }));
else if (url.endsWith('/stop')) console.log(JSON.stringify({ ok: true }));
else console.log(JSON.stringify([{ id: 'fixture-browser', health: 'ok' }]));
`, { mode: 0o755 });
  writeFileSync(join(bin, 'jq'), `#!${process.execPath}
const fs = require('node:fs'), data = JSON.parse(fs.readFileSync(0, 'utf8'));
if (process.argv.includes('-r')) console.log(data.id || '');
else process.exit(Array.isArray(data) ? (data.length ? 0 : 1) : (data.ok ? 0 : 1));
`, { mode: 0o755 });
  return root;
}

/** A real shell owns one disposable port-forward stand-in, and its EXIT trap must stop it. */
function smoke(root, env = {}) {
  const script = 'set -euo pipefail\n. "$1"\n/bin/sleep 60 &\nSMOKE_FORWARD_PID=$!\nprintf "%s" "$SMOKE_FORWARD_PID" > "$OYA_TEST_ROOT/pid"\nsmoke_test http://fixture.test synthetic-key\n';
  return spawnSync('/bin/bash', ['-c', script, 'test-smoke', LIB], {
    encoding: 'utf8', timeout: 5000,
    env: { PATH: `${root}/bin:/usr/bin:/bin`, TMPDIR: root, OYA_TEST_ROOT: root, SMOKE_POLL_S: '0', SMOKE_READY_TIMEOUT_S: '1', ...env },
  });
}

/** ESRCH proves this test did not leave its owned forward running. */
function assertClean(root) {
  const pid = Number(readFileSync(join(root, 'pid'), 'utf8'));
  assert.throws(() => process.kill(pid, 0), { code: 'ESRCH' });
  assert.deepEqual(readdirSync(root).filter((name) => name.startsWith('tmp.')), []);
}

afterEach(() => { for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true }); });

describe('deployment smoke', () => {
  for (const context of ['my-cluster', 'arn:aws:eks:us-east-1:123456789012:cluster/team']) {
    it(`deploys and removes a usable overlay for context ${context}`, () => {
      const root = fixture(), script = kubernetesFixture(root), digest = `sha256:${'a'.repeat(64)}`;
      const env = { PATH: `${root}/bin:/usr/bin:/bin`, OYA_TEST_ROOT: root };
      const result = spawnSync('/bin/bash', [script, 'deploy', '--context', context, '--image', `example/oya@${digest}`, '--browser-image', `example/browser@${digest}`, '--in-cluster-postgres'], { env, encoding: 'utf8', timeout: 10000 });
      assert.equal(result.status, 0, result.stdout + result.stderr);
      const overlay = readFileSync(join(root, 'applied-overlay'), 'utf8');
      assert.equal(basename(dirname(overlay)), '.overlays');
      const destroyed = spawnSync('/bin/bash', [script, 'destroy', '--context', context, '--yes'], { env, encoding: 'utf8', timeout: 10000 });
      assert.equal(destroyed.status, 0, destroyed.stderr);
      assert.equal(readFileSync(join(root, 'deleted-overlay'), 'utf8'), overlay);
      assert.ok(!existsSync(overlay));
    });
  }

  it('preserves the GKE smoke process PID so cancellation reaches Kubernetes cleanup', () => {
    const root = fixture(), deploy = join(root, 'deployments');
    mkdirSync(join(deploy, 'gcloud'), { recursive: true }); mkdirSync(join(deploy, 'k8s'));
    copyFileSync(new URL('../deployments/gcloud/deploy.sh', import.meta.url), join(deploy, 'gcloud', 'deploy.sh'));
    copyFileSync(LIB, join(deploy, 'lib.sh'));
    writeFileSync(join(root, 'bin', 'gcloud'), '#!/bin/sh\nexit 0\n', { mode: 0o755 });
    writeFileSync(join(deploy, 'k8s', 'deploy.sh'), '#!/bin/bash\nprintf "%s" "$$" > "$OYA_TEST_ROOT/smoke-pid"\n', { mode: 0o755 });
    const result = spawnSync('/bin/bash', [join(deploy, 'gcloud', 'deploy.sh'), 'smoke', '--project', 'fixture'], {
      env: { PATH: `${root}/bin:/usr/bin:/bin`, OYA_TEST_ROOT: root }, encoding: 'utf8',
    });
    assert.equal(result.status, 0, result.stderr);
    assert.equal(Number(readFileSync(join(root, 'smoke-pid'), 'utf8')), result.pid);
  });

  for (const phase of ['start', 'polling']) it(`cancels during browser ${phase} and stops the browser before its tunnel`, async () => {
    const root = fixture();
    writeCancellationFixture(root);
    const child = spawn(process.execPath, [join(root, 'runner.mjs')], {
      env: { PATH: `${root}/bin:/usr/bin:/bin`, OYA_TEST_ROOT: root, OYA_TEST_NODE: process.execPath, OYA_TEST_CANCEL_START: phase === 'start' ? '1' : '', TMPDIR: root }, stdio: ['ignore', 'pipe', 'pipe'],
    });
    child.stdout.resume(); child.stderr.resume();
    const end = once(child, 'close');
    for (let attempt = 0; !existsSync(join(root, 'polling')) && attempt < 300; attempt++) await new Promise((resolve) => setTimeout(resolve, 10));
    assert.ok(existsSync(join(root, 'polling')));
    child.kill('SIGTERM');
    const [code] = await end;
    assert.equal(code, 1);
    assert.ok(existsSync(join(root, 'stopped')));
    assert.ok(!existsSync(join(root, 'alive')));
    assert.deepEqual(readdirSync(root).filter((name) => name.startsWith('tmp.')), []);
  });

  it('pins image references without discarding a registry port', () => {
    const root = fixture();
    const digest = `sha256:${'a'.repeat(64)}`;
    writeFileSync(join(root, 'bin', 'docker'), `#!/bin/sh\nprintf '%s\\n' '"${digest}"'\n`, { mode: 0o755 });
    const result = spawnSync('/bin/bash', ['-c', '. "$1"; pin_image localhost:5001/acme/oya:release', 'pin-test', LIB], {
      encoding: 'utf8', env: { PATH: `${root}/bin:/usr/bin:/bin` },
    });
    assert.equal(result.status, 0, result.stderr);
    assert.equal(result.stdout.trim(), `localhost:5001/acme/oya@${digest}`);
  });

  it('waits for readiness before creating a browser and cleans its forward on failure', () => {
    const root = fixture(), result = smoke(root, { OYA_TEST_READY_FAIL: '1' });
    assert.equal(result.status, 1, result.stderr);
    assert.match(result.stderr, /readyz did not become ready/);
    assert.doesNotMatch(readFileSync(join(root, 'calls'), 'utf8'), /\/start/);
    assertClean(root);
  });

  it('stops an owned browser when navigation fails and removes header files and its forward', () => {
    const root = fixture(), result = smoke(root, { OYA_TEST_NAV_FAIL: '1' });
    assert.equal(result.status, 1, result.stderr);
    assert.match(readFileSync(join(root, 'calls'), 'utf8'), /fixture-browser\/stop/);
    assertClean(root);
  });

  it('passes only after readiness, navigation and stop, without stopping twice', () => {
    const root = fixture(), result = smoke(root);
    assert.equal(result.status, 0, result.stderr);
    const calls = readFileSync(join(root, 'calls'), 'utf8');
    assert.equal(calls.split('\n')[0], 'http://fixture.test/readyz');
    assert.equal(calls.match(/fixture-browser\/stop/g).length, 1);
    assert.match(result.stderr, /Smoke test passed/);
    assertClean(root);
  });
});

/** Run the real Kubernetes entry point with harmless tool adapters and a minimal manifest tree. */
function kubernetesFixture(root) {
  const dir = join(root, 'deployments', 'k8s'), bin = join(root, 'bin');
  mkdirSync(join(dir, 'base'), { recursive: true }); mkdirSync(join(dir, 'components', 'postgres'), { recursive: true });
  const script = join(dir, 'deploy.sh');
  copyFileSync(new URL('../deployments/k8s/deploy.sh', import.meta.url), script);
  copyFileSync(LIB, join(root, 'deployments', 'lib.sh'));
  writeFileSync(join(dir, 'base', 'control-plane.yaml'), 'apiVersion: v1\n');
  const tool = kubernetesTool();
  for (const name of ['kubectl', 'docker', 'jq', 'openssl', 'git', 'python3']) writeFileSync(join(bin, name), tool, { mode: 0o755 });
  return script;
}

/** Validate public script effects without contacting a cluster or relying on a private helper's name. */
function kubernetesTool() {
  return `#!${process.execPath}
const fs = require('node:fs'), path = require('node:path'), crypto = require('node:crypto');
const name = path.basename(process.argv[1]), args = process.argv.slice(2), root = process.env.OYA_TEST_ROOT;
if (name === 'openssl') console.log('SHA2-256(stdin)= ' + crypto.createHash('sha256').update(fs.readFileSync(0)).digest('hex'));
if (name === 'python3') console.log(path.relative(args.at(-1), args.at(-2)));
if (name === 'kubectl') {
  if (args.includes('-k')) {
    const overlay = args[args.indexOf('-k')+1];
    if (!fs.existsSync(path.resolve(overlay, '../../base/control-plane.yaml'))) process.exit(9);
    fs.writeFileSync(root + (args.includes('delete') ? '/deleted-overlay' : '/applied-overlay'), overlay);
  }
  if (args.includes('namespace') && args.includes('create')) console.log('{}');
  if (args.includes('-')) fs.readFileSync(0);
  if (args.some(arg => arg.includes('jsonpath'))) console.log('c3ludGhldGljLWtleQ==');
}
`;
}

/** Model the browser API as reachable only while the owned forward is alive. */
function writeCancellationFixture(root) {
  const runner = new URL('../packages/cli/src/install/deployment-process.ts', import.meta.url).href;
  writeFileSync(join(root, 'runner.mjs'), `import { runDeploymentScript } from ${JSON.stringify(runner)};\ntry { await runDeploymentScript(${JSON.stringify(join(root, 'smoke.sh'))}, [], ${JSON.stringify(root)}, process.env); } catch { process.exitCode = 1; }\n`);
  writeFileSync(join(root, 'forward.mjs'), `import fs from 'node:fs';\nfs.writeFileSync(process.env.OYA_TEST_ROOT+'/alive','1');\nprocess.on('SIGTERM',()=>{fs.unlinkSync(process.env.OYA_TEST_ROOT+'/alive');process.exit(0)});\nsetInterval(()=>{},1000);\n`);
  writeFileSync(join(root, 'smoke.sh'), `#!/bin/bash\nset -euo pipefail\n. ${JSON.stringify(LIB)}\n"$OYA_TEST_NODE" "$OYA_TEST_ROOT/forward.mjs" &\nSMOKE_FORWARD_PID=$!\nwhile [ ! -f "$OYA_TEST_ROOT/alive" ]; do /bin/sleep 0.01; done\nsmoke_test http://fixture.test synthetic-key\n`);
  writeFileSync(join(root, 'bin', 'curl'), `#!${process.execPath}
const fs = require('node:fs'), root = process.env.OYA_TEST_ROOT, url = process.argv.at(-1);
if (!fs.existsSync(root + '/alive')) process.exit(7);
if (url.endsWith('/readyz')) process.exit(0);
if (url.endsWith('/start')) {
  if (process.env.OYA_TEST_CANCEL_START) { fs.writeFileSync(root+'/polling','1'); setTimeout(()=>console.log(JSON.stringify({id:'fixture-browser'})),1000); }
  else console.log(JSON.stringify({id:'fixture-browser'}));
}
else if (url.endsWith('/stop')) { fs.writeFileSync(root+'/stopped','1'); console.log(JSON.stringify({ok:true})); }
else { fs.writeFileSync(root+'/polling','1'); setTimeout(()=>console.log('[]'),1000); }
`, { mode: 0o755 });
}

describe('installer bootstrap preview', () => {
  it('keeps a relative OYA_DIR anchored to the original directory after entering the checkout', () => {
    const root = fixture(), bin = join(root, 'bin'), checkout = join(root, 'custom');
    mkdirSync(join(checkout, '.git'), { recursive: true });
    for (const tool of ['git', 'node']) writeFileSync(join(bin, tool), '#!/bin/sh\nexit 0\n', { mode: 0o755 });
    writeFileSync(join(bin, 'npx'), '#!/bin/sh\nprintf "%s" "$OYA_DIR" > "$OYA_TEST_ROOT/checkout"\n', { mode: 0o755 });
    const bootstrap = fileURLToPath(new URL('../install.sh', import.meta.url));
    const result = spawnSync('/bin/sh', [bootstrap], {
      cwd: root, encoding: 'utf8', env: { PATH: `${bin}:/usr/bin:/bin`, HOME: root, OYA_TEST_ROOT: root, OYA_YES: '1', OYA_DIR: 'custom' },
    });
    assert.equal(result.status, 0, result.stderr);
    assert.equal(realpathSync(readFileSync(join(root, 'checkout'), 'utf8')), realpathSync(checkout));
  });

  it('does not clone, pull, start Docker or change directories into an absent checkout', () => {
    const root = fixture(), bin = join(root, 'bin');
    const bootstrap = fileURLToPath(new URL('../install.sh', import.meta.url));
    for (const tool of ['git', 'docker']) writeFileSync(join(bin, tool), '#!/bin/sh\nprintf called >> "$OYA_TEST_ROOT/forbidden"\n', { mode: 0o755 });
    writeFileSync(join(bin, 'node'), '#!/bin/sh\nexit 0\n', { mode: 0o755 });
    writeFileSync(join(bin, 'npx'), '#!/bin/sh\nprintf "%s\\n" "$@" > "$OYA_TEST_ROOT/npx-args"\n', { mode: 0o755 });
    const result = spawnSync('/bin/sh', [bootstrap, '--dry-run', '--target', 'ecs'], {
      cwd: root, encoding: 'utf8', env: { PATH: `${bin}:/usr/bin:/bin`, HOME: root, OYA_TEST_ROOT: root, OYA_YES: '1', OYA_DIR: join(root, 'absent') },
    });
    assert.equal(result.status, 0, result.stderr);
    assert.ok(!existsSync(join(root, 'forbidden')));
    assert.ok(!existsSync(join(root, 'absent')));
    assert.match(readFileSync(join(root, 'npx-args'), 'utf8'), /--target\necs/);
  });
});
