/**
 * `oya init` off a terminal, driven through the built CLI against a stub server.
 *
 * Piped answers are a supported way to script the wizard, blank lines included:
 * each one accepts a default on purpose. Input that is simply absent is not an
 * answer, and saving every default over a person's settings because stdin was
 * closed is the failure this pins.
 */
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { mkdtempSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import assert from 'node:assert/strict';

const CLI = new URL('../../dist/index.js', import.meta.url).pathname;

/** What the server says the key's config is: two providers, no model credential yet. */
const CONFIG = {
  providers: [
    { id: 'oya-selfhosted', label: 'Oya, self-hosted', configured: true, needs: [] },
    { id: 'cdp', label: 'Your own Chrome', configured: true, needs: [] },
  ],
  has_openai_key: false,
  chat_model: 'gpt-4o-mini',
};

/** Every request that would change something, as `METHOD path`. */
const writes = [];
const server = createServer((req, res) => {
  if (req.method !== 'GET') writes.push(`${req.method} ${req.url}`);
  req.resume();
  res.setHeader('content-type', 'application/json');
  res.end(JSON.stringify(req.method === 'GET' ? CONFIG : {}));
});
await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));

/** Runs `oya init` with `input` piped in, or with stdin closed when it is null. */
function init(input) {
  const env = {
    ...process.env,
    OYA_BASE_URL: `http://127.0.0.1:${server.address().port}`,
    OYA_API_KEY: 'test-key',
    OYA_CONFIG_HOME: mkdtempSync(join(tmpdir(), 'oya-init-')),
  };
  const child = spawn('node', [CLI, 'init'], { env, stdio: [input === null ? 'ignore' : 'pipe', 'pipe', 'pipe'] });
  let out = '';
  for (const stream of [child.stdout, child.stderr]) stream.on('data', (chunk) => (out += chunk));
  if (input !== null) child.stdin.end(input);
  return new Promise((resolve) => child.on('close', (code) => resolve({ code, out })));
}

try {
  // ── 1. No input at all: nothing is saved ───────────────────────────────────
  {
    writes.length = 0;
    const { code, out } = await init(null);
    assert.deepEqual(writes, [], 'closed stdin must not save anything');
    assert.notEqual(code, 0, 'and must not report success');
    assert.match(out, /Nothing was saved/);
    console.log('✔ init with no input saves nothing and says so');
  }

  // ── 2. Blank lines piped in: defaults accepted on purpose, and saved ───────
  {
    writes.length = 0;
    const { code, out } = await init('\n\n\n\n\n\n\n\n');
    assert.equal(code, 0, out);
    assert.ok(
      writes.some((w) => w.includes('/api/config')),
      `expected a config save, saw ${JSON.stringify(writes)}`,
    );
    console.log('✔ init with piped blank answers still saves the defaults');
  }
} finally {
  server.close();
}

console.log('\nAll init checks passed.');
