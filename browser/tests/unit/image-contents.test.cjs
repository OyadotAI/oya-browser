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
    `${target}.ts`,
    path.join(target, 'index.js'),
    path.join(target, 'index.cjs'),
    path.join(target, 'index.ts'),
  ];
  return candidates.find((c) => fs.existsSync(c) && fs.statSync(c).isFile()) || null;
}

/** A local module path in require('…'), import … from '…' or import('…'). */
const LOCAL_SPEC = /(?:require\(\s*|import\(\s*|\bfrom\s+)['"](\.{1,2}\/[^'"]+)['"]/g;

/** Every local file reachable by require() or import from the entry files. */
function reachable(entries) {
  const seen = new Set();
  const visit = (file) => {
    if (seen.has(file)) return;
    seen.add(file);
    const specs = fs.readFileSync(file, 'utf8').matchAll(LOCAL_SPEC);
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
  it('the browser image (browser/Dockerfile) includes everything its build bundles', () => {
    const files = reachable([
      path.join(BROWSER, 'src', 'main', 'main.ts'),
      path.join(BROWSER, 'src', 'preload', 'index.ts'),
    ]);
    assert.deepEqual(notCopied(files, copiedPaths(path.join(BROWSER, 'Dockerfile'), 'browser')), []);
  });

  it('the server image (Dockerfile) includes every browser file the server loads', () => {
    const entries = [
      'src/page/recording.ts',
      'src/page/render.ts',
      'src/page/queries.ts',
      'src/page/date-value.ts',
      'src/page/dialog-text.ts',
      'src/page/login-state.ts',
      'src/anonymity/apply.ts',
      'src/workflow/index.ts',
    ];
    const files = reachable(entries.map((e) => path.join(BROWSER, e)).filter((f) => fs.existsSync(f)));
    const copied = copiedPaths(path.join(REPO, 'Dockerfile'), '').map((p) =>
      p.replace(`${path.sep}server${path.sep}`, `${path.sep}server${path.sep}`),
    );
    assert.deepEqual(notCopied(files, copied), []);
  });

  it('a change to any browser path the server image copies redeploys the server (deploy-dev.yaml)', () => {
    const workflow = fs.readFileSync(path.join(REPO, '.github', 'workflows', 'deploy-dev.yaml'), 'utf8');
    const block = workflow.match(/^ {10}server:\n((?: {12}.*\n)+)/m)?.[1] || '';
    const filters = [...block.matchAll(/- '([^']+)'/g)].map((m) => m[1]);
    const covers = (src) => filters.some((f) => f === src || (f.endsWith('/**') && src.startsWith(f.slice(0, -2))));
    const sources = [...fs.readFileSync(path.join(REPO, 'Dockerfile'), 'utf8').matchAll(/^COPY (browser\/\S+) /gm)];
    assert.ok(sources.length, 'the root Dockerfile copies nothing from browser/: did the COPY lines move?');
    assert.deepEqual(
      sources.map((m) => m[1].replace(/\/$/, '/x')).filter((src) => !covers(src)),
      [],
      'the server image copies these, but changing them would not redeploy the server',
    );
  });
});
