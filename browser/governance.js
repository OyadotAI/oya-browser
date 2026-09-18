/** Managed-only browser hooks. Network isolation remains the outer enforcement boundary. */
const configuration = process.env.OYA_GOVERNANCE ? JSON.parse(process.env.OYA_GOVERNANCE) : null;
let mode = 'agent';
/**
 * Host rules: an exact lowercase name, `*.domain` (any subdomain, never the bare
 * domain), or a `*` inside a label (`*-aiplatform.googleapis.com`). A wildcard is
 * `[^.]*`, so it never crosses a label boundary, and the `*.` prefix stays a raw
 * suffix test — tightening it to a DNS-label class would stop matching real hosts
 * like `_dmarc.example.com`, which reads as hardening but is a humanHosts bypass.
 * Star count and rule length are capped by validatePolicy; without that cap these
 * patterns backtrack catastrophically.
 * Kept byte-identical to server/src/modules/control/egress.ts — the proxy and the renderer
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
function matches(host, rules) {
  host = host.toLowerCase().replace(/\.$/, '');
  return host.length <= 253 && rules.some(rule => compile(rule).test(host));
}
function allowed(raw) {
  if (!configuration) return true;
  if (raw === 'about:blank') return true;
  let url;
  try { url = new URL(raw); } catch { return false; }
  if (!['http:', 'https:', 'ws:', 'wss:'].includes(url.protocol)) return false;
  return configuration.policies.every(policy =>
    (!policy.allowedHosts || matches(url.hostname, policy.allowedHosts)) &&
    (!policy.humanHosts || !matches(url.hostname, policy.humanHosts) || mode === 'human'));
}
function install(session) {
  if (!configuration) return;
  session.webRequest.onBeforeRequest((details, callback) => callback({ cancel: !allowed(details.url) }));
  session.setPermissionRequestHandler((contents, permission, callback) => callback(false));
  session.setPermissionCheckHandler(() => false);
}
module.exports = { configuration, install, allowed, setMode(value) { mode = value === 'human' ? 'human' : 'agent'; } };
