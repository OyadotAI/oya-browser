/** Preserve the legacy tooling interfaces around minimatch's patched, nonrecursive matcher. */
const fs = require('node:fs');
const path = require('node:path');
const { createRequire } = require('node:module');

/** The reviewed upstream release; upgrades must recheck these compatibility interfaces. */
const VERSION = '11.0.4';
/** Identifies the appended adapter so repeated installs cannot wrap the parser twice. */
const MARKER = '\n/* Oya minimatch compatibility: upstream matching, legacy tooling interfaces. */\n';
/** Runs inside minimatch's CommonJS module, preserving its ESM implementation unchanged. */
function compatibility(api) {
  const parse = api.Minimatch.prototype.parse;
  api.Minimatch.prototype.parse = function (pattern) {
    const result = parse.call(this, pattern);
    if (!result || typeof result !== 'object' || typeof result.test !== 'function') return result;
    result._glob = pattern;
    result[Symbol.match] = (input) => (result.test(String(input)) ? [input] : null);
    return result;
  };
  return Object.assign(api.minimatch, api);
}

/** Only patches this install's own dependency; production installs can omit dev tooling entirely. */
function installedEntry(directory) {
  const dependency = path.join(directory, 'node_modules/minimatch/package.json');
  if (!fs.existsSync(dependency)) return null;
  const resolve = createRequire(path.join(directory, 'package.json'));
  const entry = resolve.resolve('minimatch');
  if (
    resolve('minimatch/package.json').version !== VERSION ||
    !entry.endsWith(path.join('dist', 'commonjs', 'index.js'))
  )
    throw new Error('Review the minimatch compatibility adapter before upgrading');
  return entry;
}

/** An unexpected source layout fails installation instead of silently leaving incompatible tooling. */
function patch(directory) {
  const entry = installedEntry(directory);
  if (!entry) return;
  const source = fs.readFileSync(entry, 'utf8');
  if (source.includes(MARKER)) return;
  if (!source.includes('exports.Minimatch = Minimatch;') || !source.includes('exports.minimatch = minimatch;'))
    throw new Error('Unexpected minimatch CommonJS source; review the compatibility adapter');
  fs.appendFileSync(entry, `${MARKER}module.exports = (${compatibility.toString()})(exports);\n`);
}

if (require.main === module) patch(process.cwd());
module.exports = { patch };
