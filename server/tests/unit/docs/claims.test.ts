/**
 * The public documents make claims a reader can check in one click. These
 * tests check them instead: that every relative link resolves, that the tool
 * names and counts match the code, and that the prose keeps the house style.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, existsSync, readdirSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { BROWSER_TOOL_NAMES } from '../../../src/mcp/browser-tools.ts';
import { POOL_TOOL_NAMES } from '../../../src/mcp/pool-server.ts';

/** The repository root, from this file. */
const ROOT = resolve(import.meta.dirname, '../../../..');
/** Reads a file from the repository root. */
const read = (path: string) => readFileSync(join(ROOT, path), 'utf8');
/** The documents a reader lands on, all of them tracked. */
const DOCS = [
  'README.md',
  'ARCHITECTURE.md',
  'CONTRIBUTING.md',
  'packages/sdk/README.md',
  'packages/cli/README.md',
  ...readdirSync(join(ROOT, 'docs'))
    .filter((f) => f.endsWith('.md'))
    .map((f) => `docs/${f}`),
];

/** Every markdown link target in `text` that points at a file in this repository. */
function localLinks(text: string): string[] {
  const links = [...text.matchAll(/\]\(([^)\s]+)\)/g)].map((m) => m[1]);
  return links.filter((href) => !/^(https?:|mailto:|#)/.test(href)).map((href) => href.split('#')[0]);
}

describe('public documents', () => {
  for (const doc of DOCS) {
    it(`${doc} links only to files that exist`, () => {
      const missing = localLinks(read(doc)).filter((href) => !existsSync(join(ROOT, dirname(doc), href)));
      assert.deepEqual(missing, [], `${doc} links to ${missing.join(', ')}`);
    });
  }

  it('llms.txt names the tools the pool endpoint actually serves', () => {
    const llms = read('server/src/public/llms.txt');
    const pool = llms.slice(llms.indexOf('### Pool MCP Endpoint'), llms.indexOf('### Cookie Sync'));
    for (const name of POOL_TOOL_NAMES) assert.ok(pool.includes(name), `the pool section omits ${name}`);
    const perBrowserOnly = BROWSER_TOOL_NAMES.filter((name) => !POOL_TOOL_NAMES.includes(name));
    for (const name of perBrowserOnly) {
      const offered = new RegExp(`^\\s*${name}\\b`, 'm').test(pool);
      assert.equal(offered, false, `the pool section offers ${name}, which the pool does not serve`);
    }
  });

  it('every tool name in llms.txt is a tool that exists', () => {
    const known = new Set([...BROWSER_TOOL_NAMES, ...POOL_TOOL_NAMES]);
    const headings = [...read('server/src/public/llms.txt').matchAll(/^### ([a-z_]+)\b/gm)].map((m) => m[1]);
    const phantom = headings.filter((name) => name.includes('_') && !known.has(name));
    assert.deepEqual(phantom, [], `llms.txt documents ${phantom.join(', ')}`);
  });

  it('the documents carry no em-dashes', () => {
    const offenders = [...DOCS, 'server/src/public/llms.txt'].filter((doc) => read(doc).includes('—'));
    assert.deepEqual(offenders, [], `em-dash in ${offenders.join(', ')}`);
  });
});
