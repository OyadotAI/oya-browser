/** OS launch regression: a dev bundle must boot the app without a terminal-supplied app path. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import vm from 'node:vm';
import path from 'node:path';
import { nativeDevelopmentEntry } from '../../../src/dev/native-entry.ts';

for (const argv of [['Electron'], ['Electron', 'oya://connect?key=test'], ['Electron', '--oya-native-browsing']]) {
  it(`boots the intended app and profile for ${JSON.stringify(argv)}`, () => {
    const loaded: string[] = [];
    const roots: string[] = [];
    const process = { argv: [...argv], env: {} as Record<string, string> };
    const root = path.resolve('a path/with "quotes"');
    const profile = path.resolve('test profile');
    const require = (name: string) =>
      name === 'electron' ? { app: { setAppPath: (value: string) => roots.push(value) } } : loaded.push(name);
    vm.runInNewContext(nativeDevelopmentEntry(root, profile), { require, process });
    assert.deepEqual(roots, [root]);
    assert.deepEqual(loaded, [path.join(root, 'out/main/index.js')]);
    assert.equal(process.env.OYA_USER_DATA_DIR, profile);
    assert.equal(process.argv.filter((arg) => arg === '--oya-native-browsing').length, 1);
    for (const arg of argv) assert.ok(process.argv.includes(arg), 'OS-delivered URL must remain intact');
  });
}
