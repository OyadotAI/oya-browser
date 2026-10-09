/** Native recording and window fixture launch is independent of the invoking directory and never falls back to a stock engine. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { tmpdir } from 'node:os';
import { existsSync } from 'node:fs';
/** Absolute test entry survives running from the repository, package or an unrelated folder. */
const launchers = ['recording', 'windows', 'native-runtime-worlds'];
/** Replace only the process seam in a separate Node process; no browser is launched by these unit tests. */
const source = (launcher) => `
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
for (const name of launchers) {
  const launcher = new URL(`../../integration/${name}.mjs`, import.meta.url);
  const fixture = fileURLToPath(new URL(`./${name}-electron.cjs`, launcher));
  test(`${name} launches the exact fixture and forwards CI flags from every working directory`, () => {
    const directories = [
      fileURLToPath(new URL('../../../../', import.meta.url)),
      fileURLToPath(new URL('../../../', import.meta.url)),
      tmpdir(),
    ];
    assert.ok(existsSync(fixture));
    for (const cwd of directories) {
      const run = spawnSync(process.execPath, ['--input-type=module', '-e', source(launcher)], {
        cwd,
        encoding: 'utf8',
        env: { ...process.env, OYA_NATIVE_ENGINE: '/explicit/Oya Browser' },
      });
      assert.equal(run.status, 0, run.stderr);
      assert.deepEqual(JSON.parse(run.stdout), { engine: '/explicit/Oya Browser', args: [fixture, '--no-sandbox'] });
    }
  });
  test(`${name} refuses a missing native engine instead of skipping or choosing a stock browser`, () => {
    const run = spawnSync(process.execPath, ['--input-type=module', '-e', source(launcher)], {
      encoding: 'utf8',
      env: { ...process.env, OYA_NATIVE_ENGINE: '' },
    });
    assert.notEqual(run.status, 0);
    assert.match(run.stderr, /Set OYA_NATIVE_ENGINE/);
    assert.equal(run.stdout, '');
  });
}

test('window launcher refuses legacy persona instrumentation before starting an engine', () => {
  const launcher = new URL('../../integration/windows.mjs', import.meta.url);
  const run = spawnSync(process.execPath, ['--input-type=module', '-e', source(launcher)], {
    encoding: 'utf8',
    env: { ...process.env, OYA_NATIVE_ENGINE: '/explicit/Oya Browser', OYA_WINDOWS_PERSONA_TEST: '1' },
  });
  assert.notEqual(run.status, 0);
  assert.match(run.stderr, /requires native protection/);
  assert.equal(run.stdout, '');
});
