/** Security upgrades must preserve the actual glob and archive paths used by desktop packaging. */
const assert = require('node:assert/strict');
const { test } = require('node:test');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { promisify } = require('node:util');
const glob = require('glob');
const asar = require('@electron/asar');
const { patch } = require('../../../build/minimatch-compat.cjs');

/** The package whose postinstall adapts legacy packaging callers. */
const browser = path.resolve(__dirname, '../../..');
/** A real directory tree makes missing or extra packaged files observable. */
function tree(t) {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-minimatch-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  for (const name of ['one.js', 'two.ts', '.hidden.js', 'nested/three.js', 'nested/skip.txt']) {
    fs.mkdirSync(path.dirname(path.join(directory, name)), { recursive: true });
    fs.writeFileSync(path.join(directory, name), name);
  }
  return directory;
}

test('patched minimatch preserves sync and async legacy glob selection', async (t) => {
  const cwd = tree(t);
  for (const [pattern, options, expected] of [
    ['*.js', {}, ['one.js']],
    ['*.js', { dot: true }, ['.hidden.js', 'one.js']],
    ['*.{js,ts}', {}, ['one.js', 'two.ts']],
    ['@(one|two).*', {}, ['one.js', 'two.ts']],
    ['!(*.ts)', { nodir: true }, ['one.js']],
    ['**/*.js', {}, ['nested/three.js', 'one.js']],
    ['**/*', { nodir: true, ignore: '**/*.txt' }, ['nested/three.js', 'one.js', 'two.ts']],
  ]) {
    assert.deepEqual(glob.sync(pattern, { cwd, ...options }).sort(), expected);
    assert.deepEqual((await promisify(glob)(pattern, { cwd, ...options })).sort(), expected);
  }
});

test('ASAR packs selected files and excludes ignored private files', async (t) => {
  const cwd = tree(t);
  const archive = path.join(cwd, 'fixture.asar');
  await asar.createPackageWithOptions(cwd, archive, { globOptions: { ignore: ['**/*.txt', '**/.hidden.js'] } });
  assert.deepEqual(asar.listPackage(archive).sort(), ['/nested', '/nested/three.js', '/one.js', '/two.ts']);
  assert.equal(asar.extractFile(archive, 'one.js').toString(), 'one.js');
});

test('repeated installation leaves the reviewed adapter unchanged', () => {
  const entry = require.resolve('minimatch');
  const before = fs.readFileSync(entry, 'utf8');
  patch(browser);
  patch(browser);
  assert.equal(fs.readFileSync(entry, 'utf8'), before);
});

test('production installs without development tooling require no adapter', (t) => {
  assert.doesNotThrow(() => patch(tree(t)));
});

test('an unreviewed minimatch version is refused before any source mutation', (t) => {
  const cwd = tree(t);
  const dependency = path.join(cwd, 'node_modules/minimatch');
  fs.mkdirSync(dependency, { recursive: true });
  fs.writeFileSync(path.join(dependency, 'package.json'), JSON.stringify({ version: '12.0.0', main: 'index.js' }));
  fs.writeFileSync(path.join(dependency, 'index.js'), 'module.exports = {};');
  assert.throws(() => patch(cwd), /Review the minimatch/);
  assert.equal(fs.readFileSync(path.join(dependency, 'index.js'), 'utf8'), 'module.exports = {};');
});
