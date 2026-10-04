/**
 * The managed browser's host matcher: whether a host is allowed by a policy's
 * host rules. The matcher below is a byte-for-byte copy of the one in
 * server/src/modules/control/egress.ts (the egress proxy), and
 * server/tests/integration/egress-rules.test.js compares the two texts. That
 * copy is untyped TypeScript, so this file is not type-checked (@ts-nocheck),
 * and Prettier and the size and number lint rules leave it alone.
 */
// @ts-nocheck
/**
 * Host rules: an exact lowercase name, `*.domain` (any subdomain, never the bare
 * domain), or a `*` inside a label (`*-aiplatform.googleapis.com`). A wildcard is
 * `[^.]*`, so it never crosses a label boundary, and the `*.` prefix stays a raw
 * suffix test, tightening it to a DNS-label class would stop matching real hosts
 * like `_dmarc.example.com`, which reads as hardening but is a humanHosts bypass.
 * Star count and rule length are capped by validatePolicy; without that cap these
 * patterns backtrack catastrophically.
 * Kept byte-identical to server/src/modules/control/egress.ts, the proxy and the renderer
 * disagreeing means a host one allows and the other blocks.
 */
const patterns = new Map();
/** The cached RegExp for one host rule. */
function compile(rule) {
  let pattern = patterns.get(rule);
  if (!pattern) {
    const subdomain = rule.startsWith('*.');
    const body = (subdomain ? rule.slice(2) : rule)
      .replace(/[.+?^${}()|[\]\\]/g, '\\$&')
      .replace(/\*/g, '[^.]*');
    pattern = new RegExp(`^${subdomain ? '(?:[^.]*\\.)+' : ''}${body}$`);
    // ponytail: clear-on-full, since rules are tenant-settable; LRU if it ever churns.
    if (patterns.size > 500) patterns.clear();
    patterns.set(rule, pattern);
  }
  return pattern;
}
/** Whether `host` matches any of `rules` (case- and trailing-dot-insensitive), as the proxy decides it. */
export function matchesHost(host: string, rules: readonly string[]): boolean {
  const normalized = host.toLowerCase().replace(/\.$/, '');
  return normalized.length <= 253 && rules.some((rule) => compile(rule).test(normalized));
}
