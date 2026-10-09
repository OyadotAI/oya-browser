/** Recording fixture launch is independent of the invoking directory and never falls back to a stock engine. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { tmpdir } from 'node:os';
import { existsSync } from 'node:fs';
/** Absolute test entry survives running from the repository, package or an unrelated folder. */
const launcher = new URL('../../integration/recording.mjs', import.meta.url);
/** Expected fixture is resolved beside the launcher, never under process.cwd(). */
const fixture = fileURLToPath(new URL('./recording-electron.cjs', launcher));
/** Replace only the process seam in a separate Node process; no browser is launched by these unit tests. */
const source = `
  import cp from 'node:child_process';
  import { syncBuiltinESMExports } from 'node:module';
  import { EventEmitter } from 'node:events';
  cp.spawn = (engine, args) => {
    console.log(JSON.stringify({ engine, args }));
    return new EventEmitter();
  };
  syncBuiltinESMExports();
  process.argv = ['node', 'recording.mjs', '--no-sandbox'];
  await import(${JSON.stringify(launcher.href)});
`;
test('launches the exact fixture and forwards CI flags from every working directory', () => {
  const directories = [
    fileURLToPath(new URL('../../../../', import.meta.url)),
    fileURLToPath(new URL('../../../', import.meta.url)),
    tmpdir(),
  ];
  assert.ok(existsSync(fixture));
  for (const cwd of directories) {
    const run = spawnSync(process.execPath, ['--input-type=module', '-e', source], {
      cwd,
      encoding: 'utf8',
      env: { ...process.env, OYA_NATIVE_ENGINE: '/explicit/Oya Browser' },
    });
    assert.equal(run.status, 0, run.stderr);
    assert.deepEqual(JSON.parse(run.stdout), { engine: '/explicit/Oya Browser', args: [fixture, '--no-sandbox'] });
  }
});
test('a missing native engine is a failure rather than a skipped test or stock browser fallback', () => {
  const run = spawnSync(process.execPath, ['--input-type=module', '-e', source], {
    encoding: 'utf8',
    env: { ...process.env, OYA_NATIVE_ENGINE: '' },
  });
  assert.notEqual(run.status, 0);
  assert.match(run.stderr, /Set OYA_NATIVE_ENGINE/);
  assert.equal(run.stdout, '');
});
