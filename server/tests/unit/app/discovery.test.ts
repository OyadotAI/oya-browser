/**
 * Unit tests for root discovery routes (/llms.txt, /.well-known/llms.txt,
 * /openapi.json, /.well-known/agent.json, /.well-known/agent-card.json) and
 * agent discovery manifests (server.json, plugins/oya-browser/.claude-plugin/plugin.json).
 */
import { describe, it, before, after } from 'node:test';
import assert from 'node:assert/strict';
import express from 'express';
import { createServer, type Server } from 'node:http';
import { readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { discoveryRouter } from '../../../src/app/discovery.ts';
import { DISCOVERY_CACHE_CONTROL } from '../../../src/app/constants.ts';
import { PUBLIC_DIR } from '../../../src/platform/paths.ts';
import { Status } from '../../../src/platform/http-status.ts';

/** Repository root, relative to this test file. */
const ROOT = resolve(import.meta.dirname, '../../../..');

/** Maximum allowed size in bytes for the curated /llms.txt index (10 KiB). */
const MAX_LLMS_BYTES = 10_240;

/** MCP schema 2025-12-11 maxLength for server.json description. */
const MAX_MCP_DESCRIPTION_CHARS = 100;

let server: Server;
let baseUrl: string;

before(async () => {
  const app = express();
  app.use(discoveryRouter(PUBLIC_DIR));
  app.get('/.well-known/oauth-authorization-server', (_req, res) => res.json({ issuer: 'oauth-passthrough' }));
  server = createServer(app);
  await new Promise<void>((resolveListen) => server.listen(0, '127.0.0.1', resolveListen));
  const address = server.address();
  assert.ok(address && typeof address === 'object');
  baseUrl = `http://127.0.0.1:${address.port}`;
});

after(async () => {
  await new Promise<void>((resolveClose) => server.close(() => resolveClose()));
});

describe('discoveryRouter', () => {
  it('serves identical concise llms.txt <= 10 KB across all aliases with public caching', async () => {
    const bodies: string[] = [];
    for (const path of ['/llms.txt', '/llm.txt', '/docs.txt', '/.well-known/llms.txt']) {
      const res = await fetch(`${baseUrl}${path}`);
      assert.equal(res.status, Status.OK, path);
      assert.match(res.headers.get('content-type') ?? '', /^text\/plain/);
      assert.equal(res.headers.get('cache-control'), DISCOVERY_CACHE_CONTROL, path);
      bodies.push(await res.text());
    }
    const [llms] = bodies;
    for (const body of bodies) assert.equal(body, llms);
    assert.ok(
      Buffer.byteLength(llms, 'utf8') <= MAX_LLMS_BYTES,
      `llms.txt is ${Buffer.byteLength(llms, 'utf8')} bytes`,
    );
    for (const required of [
      'curl -fsSL https://oyabrowser.com/install-desktop.sh | sh -s -- claude-code',
      'https://oyabrowser.com/install-desktop.ps1',
      'http://127.0.0.1:47321/mcp',
      'claude mcp add --transport http oya https://oyabrowser.com/mcp/pool',
      'npx skills add OyadotAI/oya-browser',
      'Oya.signup({ email })',
      'https://oyabrowser.com/llms-full.txt',
      'https://oyabrowser.com/pricing',
      'https://oyabrowser.com/openapi.json',
      'https://oyabrowser.com/.well-known/agent.json',
      'https://oyabrowser.com/feed.xml',
      'https://oyabrowser.com/vs/playwright-mcp',
      'https://oyabrowser.com/vs/chrome-devtools-mcp',
      'https://oyabrowser.com/vs/firecrawl',
      'https://oyabrowser.com/vs/stagehand',
      'https://oyabrowser.com/vs/skyvern',
    ]) {
      assert.ok(llms.includes(required), `llms.txt omits ${required}`);
    }
    assert.ok(!llms.includes('—'), 'llms.txt must not contain em-dashes');
  });

  it('serves openapi.json, agent.json and agent-card.json with public caching', async () => {
    for (const path of ['/openapi.json', '/.well-known/agent.json', '/.well-known/agent-card.json']) {
      const res = await fetch(`${baseUrl}${path}`);
      assert.equal(res.status, Status.OK, path);
      assert.match(res.headers.get('content-type') ?? '', /^application\/json/);
      assert.equal(res.headers.get('cache-control'), DISCOVERY_CACHE_CONTROL, path);
      assert.ok(JSON.parse(await res.text()));
    }
  });

  it('falls through on unknown /.well-known/* routes so OAuth discovery keeps working', async () => {
    const res = await fetch(`${baseUrl}/.well-known/oauth-authorization-server`);
    assert.equal(res.status, Status.OK);
    assert.deepEqual(await res.json(), { issuer: 'oauth-passthrough' });
  });
});

describe('agent discovery manifests', () => {
  it('keeps server.json valid against MCP 2025-12-11 schema and within the 100-char description cap', () => {
    const manifest = JSON.parse(readFileSync(join(ROOT, 'server.json'), 'utf8'));
    assert.equal(manifest.$schema, 'https://static.modelcontextprotocol.io/schemas/2025-12-11/server.schema.json');
    assert.ok(manifest.description.length <= MAX_MCP_DESCRIPTION_CHARS);
    assert.match(manifest.description, /coding agents/i);
    assert.match(manifest.description, /HIPAA/i);
    assert.equal(manifest.remotes[0].url, 'https://oyabrowser.com/mcp/pool');
  });

  it('keeps plugins/oya-browser/.claude-plugin/plugin.json valid with MCP config and broadened coding-agent metadata', () => {
    const plugin = JSON.parse(readFileSync(join(ROOT, 'plugins/oya-browser/.claude-plugin/plugin.json'), 'utf8'));
    assert.equal(plugin.mcpServers.oya.url, 'https://oyabrowser.com/mcp/pool');
    assert.match(plugin.description, /desktop app/i);
    assert.equal(plugin.mcpServers.oya.headers, undefined, 'the directory refuses a key read from the user machine');
    assert.match(plugin.privacyPolicyUrl, /^https:\/\/oyabrowser\.com\/privacy$/);
    for (const kw of ['playwright', 'playbooks', 'coding-agent', 'claude-code']) {
      assert.ok(plugin.keywords.includes(kw), `plugin.json keywords omits ${kw}`);
    }
  });

  it('keeps the plugin listing to what the Claude directory accepts automatically', () => {
    const plugin = JSON.parse(readFileSync(join(ROOT, 'plugins/oya-browser/.claude-plugin/plugin.json'), 'utf8'));
    assert.equal(plugin.displayName, 'Oya Browser', 'the listing name, which the directory compares to the manifest');
    // "Oya" and "Oya AI" were refused as too close to the listings "yoya", "yaai" and "rakkoma".
    assert.equal(plugin.author.name, 'OyadotAI');
    assert.equal(plugin.author.url, 'https://oyabrowser.com');
    assert.doesNotMatch(`${plugin.description} ${plugin.keywords.join(' ')}`, /hipaa|soc ?2|certif|baa/i);
  });
});
