/**
 * The published API description (src/public/openapi.json) against the routes the
 * server really mounts. Every /api route is either described or named below as
 * internal with the reason it is kept out, and every described route exists, so a
 * route added without a spec entry, or a spec entry left behind by a removed route,
 * fails here instead of reaching the API reference page wrong. The spec is also
 * checked for the structure a renderer needs: OpenAPI 3.0, declared path
 * parameters and tags, unique operation ids, and responses on every operation.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { ownDataDir } from '../support/data-dir.ts';

ownDataDir();
const { router } = await import('../../../src/app/api.ts');
const spec = JSON.parse(readFileSync(new URL('../../../src/public/openapi.json', import.meta.url), 'utf8'));

/** HTTP methods an OpenAPI path item can hold. */
const METHODS = ['get', 'post', 'put', 'patch', 'delete'];

/**
 * Routes index.ts mounts outside the /api router. index.ts starts the server when
 * imported, so these are pinned here by hand: keep them in step with it.
 */
const ROOT_ROUTES = [
  'GET /health',
  'GET /livez',
  'GET /readyz',
  'GET /json/version',
  'GET /json/list',
  'POST /mcp/pool',
  'GET /mcp/pool',
  'DELETE /mcp/pool',
  'POST /mcp/{browserId}',
  'GET /mcp/{browserId}',
  'DELETE /mcp/{browserId}',
];

/**
 * Routes kept out of the public spec, by path prefix or exact route, each with why.
 * The public API is what the SDK needs (packages/sdk) and the same resources' read,
 * delete and export; what only the console, the desktop app or an operator calls is
 * internal and may change without notice.
 */
const INTERNAL: [string, string][] = [
  ['/api/admin', 'The company admin page, signed in as staff.'],
  ['/api/operator', 'Server operators: drain a replica, verify the audit chain.'],
  ['/api/fleet/provision', 'Batch key minting, for the server operator token only.'],
  ['/api/billing', "The console's plan page and Stripe's webhook."],
  ['/api/marketplace', 'AWS Marketplace registration, driven by AWS.'],
  ['/api/slack', "Slack's install flow and the console's Slack settings."],
  ['/api/telemetry', 'Anonymous daily pings from self-hosted servers and Keel.'],
  ['/api/gateway', 'CDP gateway providers and recordings; public once the SDK uses them.'],
  ['/api/auth/connect', 'OAuth consent for apps such as claude.ai, a page a person clicks.'],
  ['/api/auth/mfa', "The console's own sign-in second factor."],
  ['/api/auth/oauth', "The console's social sign-in redirect."],
  ['/api/metrics', 'Prometheus scraping.'],
  ['/api/config/chatgpt', "The desktop app's ChatGPT sign-in."],
  ['/api/config/host', 'Server-wide runtime config for operators.'],
  ['/api/config/model-test', "The console's model settings check."],
  ['POST /api/auth/login', 'Console sign-in session.'],
  ['POST /api/auth/logout', 'Console sign-in session.'],
  ['POST /api/auth/refresh', 'Console sign-in session.'],
  ['POST /api/auth/signup', 'Console sign-up; agents use /api/auth/agent/signup.'],
  ['GET /api/auth/me', "The console's signed-in person."],
  ['PATCH /api/auth/me', "The console's signed-in person."],
  ['DELETE /api/auth/me', 'Account deletion, confirmed by a person in the console.'],
  ['POST /api/browsers/{browserId}/translate', "The desktop app's page translation."],
  ['/api/auth/keys', "The console's API key management, signed in as a person."],
  ['/api/auth/projects', "The console's project switcher, signed in as a person."],
  ['/api/auth/whoami', "The console's account menu."],
  ['/api/routines', 'Scheduled routines, run and reported by the desktop app.'],
  ['/api/pairing/claim', 'The desktop app redeeming a pairing code.'],
  ['/api/live', "The console's live view stream."],
  ['/api/config/site-notes', "The console's view of what the agent remembers per site."],
  ['/api/providers', "The console's provider settings."],
  ['/api/fleet', "The console's fleet overview."],
  ['/api/audit', "The console's audit page."],
  ['GET /api/pool', "The console's pool page; the SDK uses only /api/pool/cookies."],
  ['POST /api/pool/command', "The console's pool page; the SDK uses only /api/pool/cookies."],
  ['GET /api/control/audit/export', "The console's audit export."],
  ['POST /api/control/deliveries/{id}/replay', "The console's webhook delivery log."],
  ['/api/control/webhook', "The console's single project webhook setting; the SDK uses /api/control/webhooks."],
  ['POST /api/browsers/connect', "The console's attach-a-CDP-browser form."],
  ['DELETE /api/browsers/{browserId}/connection', "The console's detach for an attached CDP browser."],
  ['POST /api/browsers/disconnect-all', "The console's emergency stop."],
  ['POST /api/browsers/{browserId}/disconnect', 'Forcing a browser off the fleet, for administrators.'],
  ['POST /api/browsers/provision', 'Bulk sandbox launch from the console.'],
  ['DELETE /api/browsers/{browserId}/sandbox', "The console's sandbox cleanup."],
  ['POST /api/browsers/{browserId}/credentials/import', "The desktop app's password manager import."],
  ['POST /api/browsers/{browserId}/playbooks/{name}/play', 'The old blocking replay; the SDK submits a run instead.'],
];

/** Express `:name` as OpenAPI `{name}`. */
const toSpecPath = (path: string) => path.replace(/:(\w+)/g, '{$1}');

/** Every documented path and its prefixes, which is where a sub-router can be mounted. */
function mountCandidates(): string[] {
  const out = new Set<string>();
  for (const path of [...Object.keys(spec.paths), ...INTERNAL.map(([p]) => p.split(' ').pop()!)]) {
    const parts = path.replace(/^\/api/, '').split('/');
    for (let i = 2; i <= parts.length; i++) out.add(parts.slice(0, i).join('/'));
  }
  return [...out].sort((a, b) => b.length - a.length);
}

/**
 * Where a sub-router layer is mounted. Express 5 keeps no mount path as text, so it
 * is found by asking the layer's matcher which known prefix it takes; a mount at no
 * known prefix fails, since its routes could not be checked.
 */
function mountOf(layer, candidates: string[]): string {
  if (layer.slash) return '';
  const found = candidates.find((c) => layer.matchers[0](`${c}/__probe__`)?.path === c);
  assert.ok(found !== undefined, `a router is mounted at a path no spec or internal entry starts with`);
  return found;
}

/** Every route under `stack`, as "METHOD /path", recursing into sub-routers. */
function routesOf(stack, prefix: string, candidates: string[]): string[] {
  return stack.flatMap((layer) => {
    if (layer.route) return routeEntries(layer.route, prefix);
    if (layer.handle?.stack) return routesOf(layer.handle.stack, prefix + mountOf(layer, candidates), candidates);
    return [];
  });
}

/** One route's methods and paths as "METHOD /path" entries. */
function routeEntries(route, prefix: string): string[] {
  const paths = ([] as string[]).concat(route.path);
  const methods = Object.keys(route.methods).filter((m) => METHODS.includes(m));
  return paths.flatMap((p) => methods.map((m) => `${m.toUpperCase()} ${toSpecPath(prefix + p).replace(/\/$/, '')}`));
}

/** The internal rule that covers a route, if any. */
const internalRule = (route: string) =>
  INTERNAL.find(([rule]) => {
    if (rule.includes(' ')) return rule === route;
    const path = route.split(' ')[1];
    return path === rule || path.startsWith(`${rule}/`);
  });

/** Every operation the spec describes, as "METHOD /path". */
const described = () =>
  Object.entries(spec.paths).flatMap(([path, item]) =>
    Object.keys(item as object)
      .filter((m) => METHODS.includes(m))
      .map((m) => `${m.toUpperCase()} ${path}`),
  );

/** A `$ref` to a local component as the component, anything else as it is. */
const resolve = (node) =>
  node?.$ref
    ? node.$ref
        .slice(2)
        .split('/')
        .reduce((at, key) => at?.[key], spec)
    : node;

/** Every operation in the spec, with its path. */
const operations = () =>
  Object.entries(spec.paths).flatMap(([path, item]) =>
    Object.entries(item as object)
      .filter(([m]) => METHODS.includes(m))
      .map(([method, op]) => ({ path, method, op })),
  );

const mounted = [...new Set([...routesOf(router.stack, '/api', mountCandidates()), ...ROOT_ROUTES])];

describe('openapi.json against the mounted routes', () => {
  it('describes every public route the server mounts', () => {
    const missing = mounted.filter((r) => !described().includes(r) && !internalRule(r));
    assert.deepEqual(missing, [], 'add these to src/public/openapi.json, or to INTERNAL here with a reason');
  });

  it('describes no route the server does not mount', () => {
    const stale = described().filter((r) => !mounted.includes(r));
    assert.deepEqual(stale, [], 'remove these from src/public/openapi.json');
  });

  it('keeps internal routes out of the spec, and every internal entry still names a mounted route', () => {
    assert.deepEqual(
      described().filter((r) => internalRule(r)),
      [],
    );
    const unused = INTERNAL.filter(([rule]) => !mounted.some((r) => internalRule(r)?.[0] === rule));
    assert.deepEqual(unused, [], 'these internal entries match no route any more');
  });
});

describe('openapi.json structure', () => {
  it('is OpenAPI 3.0 with a server base at the origin, so every path is written in full', () => {
    assert.match(spec.openapi, /^3\.0\.\d+$/);
    assert.ok(spec.servers.every((s) => !new URL(s.url).pathname.replace(/\/$/, '')));
  });

  it('declares every path parameter its path names', () => {
    for (const { path, method, op } of operations()) {
      const params = [...(spec.paths[path].parameters ?? []), ...(op.parameters ?? [])].map(resolve);
      const declared = params.filter((p) => p.in === 'path').map((p) => p.name);
      const named = [...path.matchAll(/\{(\w+)\}/g)].map((m) => m[1]);
      assert.deepEqual(declared.sort(), named.sort(), `${method.toUpperCase()} ${path}`);
    }
  });

  it('gives every operation a unique id, a declared tag, a summary and responses', () => {
    const tags = new Set(spec.tags.map((t) => t.name));
    const ids = operations().map(({ op }) => op.operationId);
    assert.equal(new Set(ids).size, ids.length, 'operationIds must be unique');
    for (const { path, method, op } of operations()) {
      const where = `${method.toUpperCase()} ${path}`;
      assert.ok(op.operationId && op.summary && op.responses, where);
      assert.ok(op.tags?.length && op.tags.every((t) => tags.has(t)), `${where}: undeclared tag`);
    }
  });

  it('resolves every local $ref', () => {
    const refs = JSON.stringify(spec).match(/"\$ref":"#\/[^"]+"/g) ?? [];
    for (const ref of new Set(refs)) {
      const target = ref.slice('"$ref":"#/'.length, -1).split('/');
      assert.ok(
        target.reduce((node, key) => node?.[key], spec),
        ref,
      );
    }
  });
});
