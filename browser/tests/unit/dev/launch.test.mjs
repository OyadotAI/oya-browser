/** Development launches preserve arguments and environment without confusing the macOS waiter with the application. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

/** Every run replaces process and filesystem effects before importing the actual entry. */
const launcher = new URL('../../../src/dev/launch.ts', import.meta.url);
/** The repository's real main directory is passed as one argument. */
const directory = fileURLToPath(new URL('../../../', import.meta.url)).replace(/\/$/, '');
/** Run production launch wiring with only its native process and cache seams faked. */
function run({ platform = 'darwin', event = 'exit', args = [], imported = false } = {}) {
  const source = `
    import cp from 'node:child_process';
    import fs from 'node:fs';
    import { EventEmitter } from 'node:events';
    import { createRequire, syncBuiltinESMExports } from 'node:module';
    const require = createRequire(${JSON.stringify(launcher.href)});
    require.cache[require.resolve('electron')] = {exports:'/synthetic/Electron.app/Contents/MacOS/Electron'};
    require.cache[require.resolve('electron/package.json')] = {exports:{version:'synthetic-runtime'}};
    Object.defineProperty(process, 'platform', {value:${JSON.stringify(platform)}});
    fs.existsSync = () => true;
    fs.mkdtempSync = () => '/synthetic private logs';
    fs.writeFileSync = (file, value, options) => console.log(JSON.stringify({file, mode:options.mode}));
    cp.execFileSync = () => { throw Error('The ready branded cache must not be changed'); };
    cp.spawn = (command, args, options) => {
      console.log(JSON.stringify({command,args,shell:options.shell,marker:options.env.OYA_LAUNCH_TEST_MARKER}));
      const child = new EventEmitter();
      child.kill = signal => { console.log(JSON.stringify({killed:signal})); child.emit('exit',null,signal); };
      queueMicrotask(() => {
        if (${JSON.stringify(event)} === 'error') child.emit('error', Error('synthetic launch failure'));
        else if (${JSON.stringify(event)} === 'interrupt') process.emit('SIGINT');
        else child.emit('exit',0,null);
      });
      return child;
    };
    syncBuiltinESMExports();
    process.argv = ['node', ${JSON.stringify(imported ? '/other-entry.mjs' : fileURLToPath(launcher))}, ...${JSON.stringify(args)}];
    const entry = await import(${JSON.stringify(launcher.href)});
    if (${imported}) console.log(JSON.stringify({executable:entry.developmentExecutable()}));
  `;
  const result = spawnSync(process.execPath, ['--input-type=module', '-e', source], {
    encoding: 'utf8',
    env: { ...process.env, OYA_LAUNCH_TEST_MARKER: 'synthetic environment marker' },
    timeout: 10000,
  });
  return {
    ...result,
    records: result.stdout
      .trim()
      .split('\n')
      .filter(Boolean)
      .map((line) => (line.startsWith('{') ? JSON.parse(line) : line)),
  };
}

test('interactive macOS launches the exact branded bundle through Launch Services and preserves arguments', () => {
  const result = run({ args: ['--dev', 'https://github.com/microsoft/vscode/releases', 'argument with spaces'] });
  assert.equal(result.status, 0, result.stderr);
  const request = result.records.find((record) => record.command);
  assert.equal(request.command, '/usr/bin/open');
  assert.deepEqual(request.args.slice(0, 3), ['-n', '-W', '-a']);
  assert.match(request.args[3], /oya-runtime\/[^/]+-v1\/Oya Browser\.app$/);
  assert.deepEqual(request.args.slice(4), [
    '--stdout',
    '/synthetic private logs/stdout.log',
    '--stderr',
    '/synthetic private logs/stderr.log',
    '--args',
    directory,
    '--dev',
    'https://github.com/microsoft/vscode/releases',
    'argument with spaces',
  ]);
});

test('launch environment stays in the process environment without shell interpretation or credential arguments', () => {
  const result = run();
  const request = result.records.find((record) => record.command);
  assert.equal(request.shell, false);
  assert.equal(request.marker, 'synthetic environment marker');
  assert.equal(
    request.args.some((arg) => arg.includes('synthetic environment marker')),
    false,
  );
});

test('private separate startup logs are announced instead of silently dropping application output', () => {
  const result = run();
  assert.deepEqual(
    result.records.filter((record) => record.file).map((record) => record.mode),
    [0o600, 0o600],
  );
  assert.match(result.stderr, /Oya development logs: \/synthetic private logs/);
  assert.match(result.stderr, /Ctrl\+C stops only this launch waiter/);
});

for (const platform of ['linux', 'win32']) {
  test(`${platform} retains direct child launching`, () => {
    const result = run({ platform, args: ['--dev', 'argument with spaces'] });
    assert.equal(result.status, 0, result.stderr);
    assert.equal(result.records.length, 1);
    assert.equal(result.records[0].command, '/synthetic/Electron.app/Contents/MacOS/Electron');
    assert.deepEqual(result.records[0].args, [directory, '--dev', 'argument with spaces']);
  });
}

test('importing the executable helper neither launches the browser nor changes its ready cache', () => {
  const result = run({ imported: true });
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.records.length, 1);
  assert.match(result.records[0].executable, /Oya Browser\.app\/Contents\/MacOS\/Electron$/);
  assert.equal(result.stderr, '');
});

test('prepare prints only the direct executable without launching or allocating logs', () => {
  const result = run({ args: ['--prepare'] });
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.records.length, 1);
  assert.match(result.records[0], /Oya Browser\.app\/Contents\/MacOS\/Electron$/);
  assert.equal(result.stderr, '');
});

test('a launch failure is visible and reports an unsuccessful launcher result', () => {
  const result = run({ event: 'error' });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /synthetic launch failure/);
});

test('interrupting macOS targets only the waiter and does not signal or identify an Oya application', () => {
  const result = run({ event: 'interrupt' });
  assert.equal(result.status, 1);
  assert.deepEqual(
    result.records.filter((record) => record.killed),
    [{ killed: 'SIGINT' }],
  );
});
