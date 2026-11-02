/**
 * Writes vectors.json: governed-egress host-rule verdicts computed by Oya's own TypeScript matcher,
 * so Mars's Rust port (crates/link/src/egress.rs) and the TypeScript copies are held to the same
 * cases. A Rust port cannot be byte-identical to the TypeScript text, so this file replaces that
 * invariant across languages. `--check` re-runs every vector through both TypeScript copies
 * (server/src/modules/control/egress.ts and browser/src/main/identity/host-rules.ts) and fails on
 * any disagreement, which is the TypeScript half of "both implementations pass the same file".
 *
 * Run from anywhere: node mars/fixtures/egress/generate.mjs [--check]
 */
import { readFileSync, writeFileSync } from 'node:fs';

const root = new URL('../../../', import.meta.url);
const out = new URL('./vectors.json', import.meta.url);
const copies = ['server/src/modules/control/egress.ts', 'browser/src/main/identity/host-rules.ts'];

/** The shared compile() block and the normalisation around it, taken from the source text itself. */
function matcherFrom(file) {
  const text = readFileSync(new URL(file, root), 'utf8');
  const block = text.match(/const patterns = new Map\(\);[\s\S]*?\n}\n/)?.[0];
  if (!block) throw Error(`compile() block not found in ${file}`);
  const body = `${block}
    return (host, rules) => {
      const normalized = host.toLowerCase().replace(/\\.$/, '');
      return normalized.length <= 253 && rules.some((rule) => compile(rule).test(normalized));
    };`;
  return new Function(body)();
}

/** Rule sets: the server test's equivalence sets, mid-label wildcards and escaped metacharacters. */
const RULES = [
  ['*.example.com'],
  ['example.com'],
  ['*.example.com', 'example.com'],
  ['evil.com'],
  ['*-aiplatform.googleapis.com'],
  ['a.example.com'],
  ['*'],
  ['*.*'],
  ['*.com'],
  ['api-*.example.com'],
  ['*-*-*.example.com'],
  ['a*b.example.com'],
  ['*.*.example.com'],
  ['a+b.example.com'],
  ['(x).example.com'],
  ['a?.example.com'],
  ['x$.example.com'],
  ['[ab].example.com'],
  ['a|b.example.com'],
  ['^a.example.com'],
  ['a{2}.example.com'],
  ['back\\slash.example.com'],
  ['UPPER.example.com'],
  ['localhost'],
  ['1.2.3.4'],
  ['*.example.xn--p1ai'],
  ['example.com.'],
  [''],
  ['*.'],
  [],
];

/** Hosts: the server test's list plus case, trailing dots, label crossing, metacharacters and length. */
const HOSTS = [
  'a.example.com',
  'a.b.example.com',
  'foo_bar.example.com',
  '_dmarc.example.com',
  '.example.com',
  'a..example.com',
  'example.com',
  'xample.com',
  'notexample.com',
  'EXAMPLE.COM',
  'A.Example.Com',
  'a.example.com.',
  'a.example.com..',
  'evil.com',
  'example.com.evil.com',
  'us-central1-aiplatform.googleapis.com',
  'europe-west4-aiplatform.googleapis.com',
  '-aiplatform.googleapis.com',
  'a.b-aiplatform.googleapis.com',
  'aiplatform.googleapis.com.evil.com',
  'aXexample.com',
  'api-v2.example.com',
  'api-.example.com',
  'api.example.com',
  'x.api-v2.example.com',
  'a-b-c.example.com',
  'a-b.example.com',
  'ab.example.com',
  'axxb.example.com',
  'a.b.example.com',
  'x.y.example.com',
  'a+b.example.com',
  'aab.example.com',
  '(x).example.com',
  'x.example.com',
  'a?.example.com',
  'a.example.com',
  'x$.example.com',
  '[ab].example.com',
  'b.example.com',
  'a|b.example.com',
  '^a.example.com',
  'a{2}.example.com',
  'aa.example.com',
  'back\\slash.example.com',
  'upper.example.com',
  'localhost',
  'LOCALHOST.',
  '1.2.3.4',
  '1.2.3.45',
  'a.example.xn--p1ai',
  'example.xn--p1ai',
  'com',
  '',
  '.',
  'a',
  'ä.example.com',
  'Ä.EXAMPLE.COM',
  'İ.example.com',
  `${'a'.repeat(241)}.example.com`,
  `${'a'.repeat(242)}.example.com`,
  `${'a'.repeat(242)}.example.com.`,
  `${'ä'.repeat(241)}.example.com`,
];

/** Every host against every rule set, with the verdict from the first TypeScript copy. */
function vectors(match) {
  return RULES.flatMap((rules) => HOSTS.map((host) => ({ host, rules, allowed: match(host, rules) })));
}

const [server, browser] = copies.map(matcherFrom);
if (process.argv.includes('--check')) {
  const file = JSON.parse(readFileSync(out, 'utf8'));
  const wrong = file.vectors.filter((v) => server(v.host, v.rules) !== v.allowed || browser(v.host, v.rules) !== v.allowed);
  if (wrong.length) throw Error(`TypeScript matchers disagree with ${wrong.length} vectors: ${JSON.stringify(wrong[0])}`);
  console.log(`egress vectors: ${file.vectors.length} cases agree with both TypeScript copies`);
} else {
  const source = 'node mars/fixtures/egress/generate.mjs (from server/src/modules/control/egress.ts)';
  const lines = vectors(server).map((v) => '  ' + JSON.stringify(v));
  writeFileSync(out, `{"source": ${JSON.stringify(source)}, "vectors": [\n${lines.join(',\n')}\n]}\n`);
  console.log(`wrote ${out.pathname}`);
}
