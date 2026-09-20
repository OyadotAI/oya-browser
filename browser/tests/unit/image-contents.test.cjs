/**
 * The container images copy an explicit list of files, so a new module that is
 * required but not copied crashes the image at start while every local test
 * passes. These tests walk the requires and check each file ships.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const BROWSER = path.join(__dirname, '..', '..');
const REPO = path.join(BROWSER, '..');

/** The source paths a Dockerfile's COPY lines take, relative to `base`. */
function copiedPaths(dockerfile, base) {
  const lines = [...fs.readFileSync(dockerfile, 'utf8').matchAll(/^COPY (?!--)(.+) \S+$/gm)];
  return lines.flatMap((m) => m[1].split(/\s+/)).map((p) => path.join(REPO, base, p.replace(/\/$/, '')));
}

/** Resolves a relative require the way Node would, or null for a package. */
function resolveLocal(from, spec) {
  const target = path.join(path.dirname(from), spec);
  const candidates = [
    target,
    `${target}.js`,
    `${target}.cjs`,
    path.join(target, 'index.js'),
    path.join(target, 'index.cjs'),
  ];
  return candidates.find((c) => fs.existsSync(c) && fs.statSync(c).isFile()) || null;
}

/** Every local file reachable by require() from the entry files. */
function reachable(entries) {
  const seen = new Set();
  const visit = (file) => {
    if (seen.has(file)) return;
    seen.add(file);
    const specs = fs.readFileSync(file, 'utf8').matchAll(/require\(\s*['"](\.{1,2}\/[^'"]+)['"]\s*\)/g);
    for (const [, spec] of specs) {
      const next = resolveLocal(file, spec);
      if (next) visit(next);
    }
  };
  entries.forEach(visit);
  return [...seen];
}

/** Files among `files` that none of the copied paths include. */
const notCopied = (files, copied) => files.filter((f) => !copied.some((c) => f === c || f.startsWith(c + path.sep)));

describe('container images ship every required file', () => {
  it('the browser image (browser/Dockerfile) includes everything main.js and preload.js load', () => {
    const files = reachable([path.join(BROWSER, 'main.js'), path.join(BROWSER, 'preload.js')]);
    assert.deepEqual(notCopied(files, copiedPaths(path.join(BROWSER, 'Dockerfile'), 'browser')), []);
  });

  it('the server image (Dockerfile) includes every browser file the server loads', () => {
    const entries = ['scripts/recording.cjs', 'login-state.js', 'anonymity/apply.js', 'scripts/workflow.cjs'];
    const files = reachable(entries.map((e) => path.join(BROWSER, e)).filter((f) => fs.existsSync(f)));
    const copied = copiedPaths(path.join(REPO, 'Dockerfile'), '').map((p) =>
      p.replace(`${path.sep}server${path.sep}`, `${path.sep}server${path.sep}`),
    );
    assert.deepEqual(notCopied(files, copied), []);
  });
});
