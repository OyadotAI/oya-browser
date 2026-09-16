/**
 * Host-rule matching for governed egress. The risk in adding wildcards was never that
 * the new rules fail — it was that the old ones silently stop matching, which for
 * `humanHosts` drops a human-control requirement rather than denying anything. So the
 * core of this file is an equivalence check against the matcher that shipped before.
 */
import assert from 'node:assert';
import { readFileSync } from 'node:fs';
import { allowedHost } from './src/control/egress.js';
import { validatePolicy } from './src/control/service.js';

/** The pre-wildcard matcher, kept verbatim as the oracle. */
const previous = (host, rules) => {
  const normalized = host.toLowerCase().replace(/\.$/, '');
  return rules.some(rule => rule.startsWith('*.') ? normalized.endsWith(rule.slice(1)) && normalized !== rule.slice(2) : normalized === rule);
};

// Underscores and empty labels are not DNS-label shaped but `new URL()` hands them
// through verbatim, and the old suffix test matched them.
const hosts = ['a.example.com', 'a.b.example.com', 'foo_bar.example.com', '_dmarc.example.com',
  '.example.com', 'a..example.com', 'example.com', 'xample.com', 'notexample.com',
  'EXAMPLE.COM', 'a.example.com.', 'evil.com'];
for (const rules of [['*.example.com'], ['example.com'], ['*.example.com', 'example.com'], ['evil.com']]) {
  for (const host of hosts) {
    assert.equal(allowedHost(host, rules), previous(host, rules),
      `rule ${JSON.stringify(rules)} disagrees with the previous matcher on ${host}`);
  }
}

// `*.` still excludes the bare domain, which is the whole point of that form.
assert.equal(allowedHost('example.com', ['*.example.com']), false, '*.domain excludes the bare domain');
assert.equal(allowedHost('a.example.com', ['*.example.com']), true, '*.domain matches a subdomain');

// The rule this work exists for.
const vertex = ['*-aiplatform.googleapis.com'];
assert.equal(allowedHost('us-central1-aiplatform.googleapis.com', vertex), true, 'mid-label wildcard matches a region host');
assert.equal(allowedHost('europe-west4-aiplatform.googleapis.com', vertex), true, 'mid-label wildcard matches another region');
// A wildcard is [^.]*, so it cannot swallow a dot and reach an attacker-controlled label.
assert.equal(allowedHost('a.b-aiplatform.googleapis.com', vertex), false, 'a wildcard does not cross a label boundary');
assert.equal(allowedHost('aiplatform.googleapis.com.evil.com', vertex), false, 'a suffix impostor is denied');

// Regex metacharacters in a rule are escaped, not interpreted.
assert.equal(allowedHost('aXexample.com', ['a.example.com']), false, 'a literal dot is escaped');

validatePolicy({ allowedHosts: ['*-aiplatform.googleapis.com'] });
// Accepted before this change, so they must still be accepted: a TLD-shaped anchor
// would have quietly broken every one of these on the operator's next save.
for (const rule of ['localhost', '1.2.3.4', 'example.xn--p1ai', '*.example.xn--p1ai', 'a'])
  validatePolicy({ allowedHosts: [rule] });

const rejects = (policy, why) => assert.throws(() => validatePolicy(policy), /invalid|must contain/i, why);
rejects({ allowedHosts: ['*'.repeat(12) + 'x.example.com'] }, 'a rule with 12 stars is refused before it can backtrack');
rejects({ allowedHosts: ['*'.repeat(4) + 'x.example.com'] }, 'more than three stars is refused');
rejects({ allowedHosts: ['a'.repeat(300) + '.com'] }, 'an overlong rule is refused');
rejects({ allowedHosts: Array.from({ length: 101 }, (_, i) => `h${i}.example.com`) }, 'more than 100 rules is refused');
rejects({ allowedHosts: ['UPPER.example.com'] }, 'an uppercase rule is refused');
rejects({ allowedHosts: [] }, 'an empty rule list is refused');

// The cap has to leave the pathological case fast, not merely rejected.
const started = Date.now();
allowedHost('a'.repeat(2000) + '.example.com', ['*-*-*.example.com']);
assert.ok(Date.now() - started < 1000, 'three stars stay linear on a long hostname');

// governance.js is CJS, runs in Electron and exports no matcher, so the two copies
// cannot be compared by calling them. Compare the source instead: the failure mode this
// guards against is one copy being edited and the other left behind.
const extract = (file) => readFileSync(new URL(file, import.meta.url), 'utf8')
  .match(/const patterns = new Map\(\);[\s\S]*?\n}\n/)?.[0];
const [proxy, renderer] = ['./src/control/egress.js', '../browser/governance.js'].map(extract);
assert.ok(proxy, 'the compile() block is still findable in egress.js');
assert.equal(renderer, proxy, 'browser/governance.js and server/src/control/egress.js must share one matcher');

console.log('Egress host rules passed: equivalence with the previous matcher, *.domain excludes the bare domain, mid-label wildcards, no label crossing, star/length/count caps, punycode and IP-literal rules still accepted, proxy and renderer copies identical.');
