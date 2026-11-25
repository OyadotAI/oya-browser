/** Drive the built installer against isolated fake tools and example scripts, never real cloud resources. */
import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync, spawn } from 'node:child_process';
import {
  mkdtempSync,
  mkdirSync,
  writeFileSync,
  readFileSync,
  existsSync,
  readdirSync,
  rmSync,
  statSync,
} from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { once } from 'node:events';

const CLI = new URL('../../dist/index.js', import.meta.url).pathname;
const scratch = [];
const PLANS = {
  docker: { version: 2, target: 'docker', options: { domain: 'localhost' } },
  k8s: {
    version: 2,
    target: 'k8s',
    options: { context: 'my-cluster', namespace: 'team', registry: 'localhost:5001/oya', 'in-cluster-postgres': true },
  },
  ecs: { version: 2, target: 'ecs', options: { region: 'us-west-2', stack: 'team', 'http-only': true } },
  gcloud: { version: 2, target: 'gcloud', options: { project: 'my-project', region: 'us-central1', name: 'team' } },
};

/** A complete fake checkout plus version/identity probes that cannot reach the network. */
function fixture(target) {
  const root = mkdtempSync(join(tmpdir(), 'oya-deploy-'));
  scratch.push(root);
  const bin = join(root, 'bin'),
    dir = join(root, 'deployments', target);
  mkdirSync(bin);
  mkdirSync(dir, { recursive: true });
  mkdirSync(join(root, 'server', 'src'), { recursive: true });
  writeFileSync(join(root, 'server', 'src', 'index.ts'), '// fixture');
  writeFileSync(join(root, 'docker-compose.yml'), 'services: {}');
  writeFileSync(join(root, '.env'), 'OYA_PROFILE_SECRET=sentinel\n');
  installFakeTools(root, bin);
  writeFileSync(join(dir, 'deploy.sh'), fakeDeployment());
  const config = join(root, 'plan.json');
  writeFileSync(config, JSON.stringify(PLANS[target]));
  const env = { PATH: `${bin}:/bin:/usr/bin`, HOME: root, OYA_CONFIG_HOME: root, OYA_TEST_ROOT: root };
  return { root, config, env };
}

/** Record argv separately from secrets and exercise phase ordering and cancellation. */
function fakeDeployment() {
  return `#!/bin/bash
printf '%s\\n' "$1" >> "$OYA_TEST_ROOT/phases"
printf '<%s>' "$@" >> "$OYA_TEST_ROOT/arguments"
if [ -n "\${OYA_DEPLOY_DATABASE_URL:-}" ]; then printf received > "$OYA_TEST_ROOT/database-received"; fi
if [ "$1" = "\${OYA_TEST_FAIL:-}" ]; then exit 9; fi
if [ "$1" = "\${OYA_TEST_HOLD:-}" ]; then
  if [ "\${OYA_TEST_TUNNEL:-}" = 1 ]; then
    /bin/sleep 60 &
    forward=$!
    printf '%s' "$forward" > "$OYA_TEST_ROOT/forward-pid"
  fi
  trap 'if [ -n "\${forward:-}" ] && kill -0 "$forward" 2>/dev/null; then printf available > "$OYA_TEST_ROOT/cleanup-tunnel"; fi; printf stopped > "$OYA_TEST_ROOT/stopped"; exit 0' TERM
  printf ready > "$OYA_TEST_ROOT/started"
  while true; do /bin/sleep 1; done
fi
`;
}

/** Each prerequisite is a harmless executable; there is no fallback to a real cloud CLI. */
function installFakeTools(root, bin) {
  const body = `#!${process.execPath}
const fs = require('node:fs');
const path = require('node:path');
const name = path.basename(process.argv[1]);
fs.appendFileSync(process.env.OYA_TEST_ROOT + '/tools', name + ' ' + process.argv.slice(2).join(' ') + '\\n');
if (process.env.OYA_TEST_MISSING === name) process.exit(1);
if (name === 'aws' && process.argv.includes('sts')) console.log('123456789012');
else if (name === 'gcloud') console.log('my-project');
else console.log('2.25.0');
`;
  for (const tool of [
    'git',
    'docker',
    'jq',
    'openssl',
    'curl',
    'aws',
    'gcloud',
    'kubectl',
    'python3',
    'gke-gcloud-auth-plugin',
  ])
    writeFileSync(join(bin, tool), body, { mode: 0o755 });
}

/** Run replay without stdin; accidentally asking any question will fail the scenario. */
function install(f, args = [], env = {}) {
  return spawnSync(process.execPath, [CLI, 'install', '--config', f.config, ...args], {
    cwd: f.root,
    env: { ...f.env, ...env },
    encoding: 'utf8',
    timeout: 10_000,
  });
}

afterEach(() => {
  for (const root of scratch.splice(0)) rmSync(root, { recursive: true, force: true });
});

describe('deployment installer', () => {
  for (const target of Object.keys(PLANS)) {
    it(`deploys and verifies the ${target} example with its explicit destination`, () => {
      const f = fixture(target),
        result = install(f);
      assert.equal(result.status, 0, result.stdout + result.stderr);
      assert.equal(readFileSync(join(f.root, 'phases'), 'utf8'), 'deploy\nsmoke\n');
      assert.match(result.stdout, /Oya is ready/);
      assert.equal(readFileSync(join(f.root, '.env'), 'utf8'), 'OYA_PROFILE_SECRET=sentinel\n');
      const saved = JSON.parse(readFileSync(join(f.root, 'oya-install.json'), 'utf8'));
      assert.equal(saved.target, target);
      assert.equal(statSync(join(f.root, 'oya-install.json')).mode & 0o777, 0o600);
      for (const [key, value] of Object.entries(PLANS[target].options))
        if (value) assert.ok(readFileSync(join(f.root, 'arguments'), 'utf8').includes(`<--${key}>`));
    });
  }

  it('previews without writes, cloud credential calls, scripts or secret output', () => {
    const f = fixture('k8s'),
      original = readdirSync(f.root);
    const plan = { ...PLANS.k8s, options: { ...PLANS.k8s.options, 'in-cluster-postgres': false } };
    writeFileSync(f.config, JSON.stringify(plan));
    const result = install(f, ['--dry-run'], { OYA_DEPLOY_DATABASE_URL: 'postgres://u:DO-NOT-PRINT@db/oya' });
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /Preview only/);
    assert.ok(!result.stdout.includes('DO-NOT-PRINT'));
    assert.deepEqual(
      readdirSync(f.root).filter((name) => name !== 'tools'),
      original,
    );
    assert.doesNotMatch(readFileSync(join(f.root, 'tools'), 'utf8'), /get namespace/);
  });

  it('previews the bootstrap checkout destination when OYA_DIR does not exist', () => {
    const f = fixture('docker'),
      destination = join(f.root, 'requested-checkout');
    rmSync(join(f.root, 'server'), { recursive: true });
    rmSync(join(f.root, 'docker-compose.yml'));
    const result = install(f, ['--dry-run'], { OYA_DIR: destination });
    assert.equal(result.status, 0, result.stderr);
    assert.ok(result.stdout.includes(`to ${destination}`));
    assert.ok(!existsSync(destination));
  });

  it('passes a managed database only through the child environment', () => {
    const f = fixture('k8s');
    writeFileSync(
      f.config,
      JSON.stringify({ ...PLANS.k8s, options: { ...PLANS.k8s.options, 'in-cluster-postgres': false } }),
    );
    const result = install(f, [], { OYA_DEPLOY_DATABASE_URL: 'postgres://u:DO-NOT-PRINT@db/oya' });
    assert.equal(result.status, 0, result.stderr);
    assert.equal(readFileSync(join(f.root, 'database-received'), 'utf8'), 'received');
    for (const file of ['arguments', 'oya-install.json'])
      assert.doesNotMatch(readFileSync(join(f.root, file), 'utf8'), /DO-NOT-PRINT/);
    assert.doesNotMatch(result.stdout + result.stderr, /DO-NOT-PRINT/);
  });

  for (const phase of ['deploy', 'smoke']) {
    it(`withholds success when ${phase} fails and retains a scoped recovery plan`, () => {
      const f = fixture('ecs'),
        result = install(f, [], { OYA_TEST_FAIL: phase });
      assert.equal(result.status, 1);
      assert.doesNotMatch(result.stdout, /Oya is ready/);
      assert.match(result.stdout, /verification incomplete/);
      assert.match(result.stdout, /--stack.*team/);
      assert.equal(readFileSync(join(f.root, 'phases'), 'utf8'), phase === 'deploy' ? 'deploy\n' : 'deploy\nsmoke\n');
      assert.ok(existsSync(join(f.root, 'oya-install.json')));
    });
  }

  it('fails missing preflight tools before any resource creation and retains answers', () => {
    const f = fixture('ecs'),
      result = install(f, [], { OYA_TEST_MISSING: 'aws' });
    assert.equal(result.status, 1);
    assert.match(result.stdout, /Install AWS CLI/);
    assert.ok(!existsSync(join(f.root, 'phases')));
    assert.ok(existsSync(join(f.root, 'oya-install.json')));
  });

  it('refuses missing database credentials without prompting or running the example', () => {
    const f = fixture('k8s');
    writeFileSync(
      f.config,
      JSON.stringify({ ...PLANS.k8s, options: { ...PLANS.k8s.options, 'in-cluster-postgres': false } }),
    );
    const result = install(f);
    assert.equal(result.status, 1);
    assert.match(result.stderr, /OYA_DEPLOY_DATABASE_URL/);
    assert.ok(!existsSync(join(f.root, 'phases')));
  });

  it('rejects malformed plans and conflicting targets before invoking tools', () => {
    const f = fixture('ecs');
    let result = install(f, ['--target', 'gcloud']);
    assert.equal(result.status, 1);
    assert.match(result.stderr, /conflicts/);
    writeFileSync(f.config, JSON.stringify({ version: 42, target: 'ecs', options: {} }));
    result = install(f);
    assert.equal(result.status, 1);
    assert.match(result.stderr, /configuration version/);
    assert.ok(!existsSync(join(f.root, 'tools')));
    assert.ok(!existsSync(join(f.root, 'phases')));
  });

  it('rejects malformed environment database credentials before the example runs', () => {
    const f = fixture('k8s');
    writeFileSync(
      f.config,
      JSON.stringify({ ...PLANS.k8s, options: { ...PLANS.k8s.options, 'in-cluster-postgres': false } }),
    );
    const result = install(f, [], { OYA_DEPLOY_DATABASE_URL: 'https://DO-NOT-PRINT@db' });
    assert.equal(result.status, 1);
    assert.doesNotMatch(result.stdout + result.stderr, /DO-NOT-PRINT/);
    assert.ok(!existsSync(join(f.root, 'phases')));
  });

  it('shows all four working choices before destination questions', () => {
    const f = fixture('docker');
    const result = spawnSync(process.execPath, [CLI, 'install', '--dry-run'], {
      cwd: f.root,
      env: f.env,
      input: '1\n\n1\n',
      encoding: 'utf8',
    });
    assert.equal(result.status, 0, result.stderr);
    for (const name of ['Docker', 'Kubernetes', 'Amazon ECS', 'Google Cloud']) assert.ok(result.stdout.includes(name));
    assert.doesNotMatch(result.stdout, /not in this build|coming soon|Which database|Which LLM/);
  });

  it('cancels the owned script and never starts smoke, even when its TERM trap exits zero', async () => {
    const f = fixture('ecs');
    const child = spawn(process.execPath, [CLI, 'install', '--config', f.config], {
      cwd: f.root,
      env: { ...f.env, OYA_TEST_HOLD: 'deploy' },
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    let output = '';
    child.stdout.on('data', (data) => {
      output += data;
    });
    child.stderr.resume();
    const end = once(child, 'close');
    for (let attempt = 0; !existsSync(join(f.root, 'started')) && attempt < 200; attempt++)
      await new Promise((resolve) => setTimeout(resolve, 10));
    assert.ok(existsSync(join(f.root, 'started')), 'fixture must enter the deployment');
    child.kill('SIGTERM');
    const [code] = await end;
    assert.equal(code, 1);
    assert.ok(existsSync(join(f.root, 'stopped')));
    assert.equal(readFileSync(join(f.root, 'phases'), 'utf8'), 'deploy\n');
    assert.doesNotMatch(output, /Oya is ready/);
  });

  it('keeps the verification tunnel alive for script cleanup, then removes remaining children', async () => {
    const f = fixture('ecs');
    const child = spawn(process.execPath, [CLI, 'install', '--config', f.config], {
      cwd: f.root,
      env: { ...f.env, OYA_TEST_HOLD: 'smoke', OYA_TEST_TUNNEL: '1' },
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    child.stdout.resume();
    child.stderr.resume();
    const end = once(child, 'close');
    for (let attempt = 0; !existsSync(join(f.root, 'started')) && attempt < 200; attempt++)
      await new Promise((resolve) => setTimeout(resolve, 10));
    assert.ok(existsSync(join(f.root, 'started')));
    child.kill('SIGTERM');
    const [code] = await end;
    assert.equal(code, 1);
    assert.equal(readFileSync(join(f.root, 'cleanup-tunnel'), 'utf8'), 'available');
    const pid = Number(readFileSync(join(f.root, 'forward-pid'), 'utf8'));
    for (let attempt = 0; attempt < 100; attempt++) {
      try {
        process.kill(pid, 0);
      } catch {
        return;
      }
      await new Promise((resolve) => setTimeout(resolve, 10));
    }
    assert.fail('cancelled script left its owned tunnel running');
  });
});
