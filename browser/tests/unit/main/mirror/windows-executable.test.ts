/** Windows installation discovery is hermetic and independent of the test host OS. */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { windowsExecutable } from '../../../../src/main/mirror/windows-executable.ts';
import { windowsDefaultFamily } from '../../../../src/main/mirror/locate.ts';
const relative = 'Google\\Chrome\\Application\\chrome.exe';
const env = {
  LOCALAPPDATA: 'C:\\Users\\Alex\\AppData\\Local',
  ProgramFiles: 'C:\\Program Files',
  'ProgramFiles(x86)': 'C:\\Program Files (x86)',
};
/** Fake registration and file existence for one installation. */
const located = (file: string, registry = (_key: string) => '') =>
  windowsExecutable(relative, { env, registry, exists: (candidate) => candidate === file });
describe('Windows browser discovery', () => {
  for (const root of Object.values(env))
    it(`finds Chrome under ${root}`, () => {
      const file = `${root}\\${relative}`;
      assert.equal(located(file), file);
    });
  it('prefers registered custom installations and expands environment variables', () => {
    const file = `${env.LOCALAPPDATA}\\Custom\\chrome.exe`;
    assert.equal(
      located(file, () => '%LOCALAPPDATA%\\Custom\\chrome.exe'),
      file,
    );
  });
  it('reports a missing executable instead of attempting an invalid spawn', () => {
    assert.throws(() => located(''), /Could not find chrome.exe/);
  });
  it('recognizes Edge HTTPS ProgIds as the default browser', () => {
    assert.equal(windowsDefaultFamily('ProgId REG_SZ MSEdgeHTM'), 'edge');
  });
});
