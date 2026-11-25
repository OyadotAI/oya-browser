/**
 * Unit tests for the desktop install scripts in ui/public: they parse, refuse an agent name they
 * do not know before reaching the network, name exactly the agents the app can connect, and only
 * ever ask the app to connect: never with a token or an address of their own.
 */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { chmodSync, mkdtempSync, readFileSync, rmSync, writeFileSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const SH = new URL('../ui/public/install-desktop.sh', import.meta.url).pathname;
const PS1 = readFileSync(new URL('../ui/public/install-desktop.ps1', import.meta.url), 'utf8');
const AGENTS_TS = readFileSync(new URL('../browser/src/main/local-mcp/agents.ts', import.meta.url), 'utf8');

/** The agent ids the app can connect, read from its AGENTS table. */
function appAgents() {
  const table = AGENTS_TS.slice(AGENTS_TS.indexOf('export const AGENTS'));
  const body = table.slice(table.indexOf('{') + 1, table.indexOf('};'));
  return [...body.matchAll(/^\s*'?([a-z-]+)'?:/gm)].map((m) => m[1]).sort();
}

/** A script without its comment lines, so a comment saying what it never does is not mistaken for doing it. */
const code = (script) =>
  script
    .split('\n')
    .filter((line) => !line.trim().startsWith('#'))
    .join('\n');

/** Runs the shell script with a PATH whose curl, open and hdiutil only record that they ran. */
function runSh(args) {
  const bin = mkdtempSync(join(tmpdir(), 'oya-install-test-'));
  const ran = join(bin, 'ran');
  for (const tool of ['curl', 'open', 'hdiutil']) {
    writeFileSync(join(bin, tool), `#!/bin/sh\necho ${tool} >> "${ran}"\nexit 1\n`);
    chmodSync(join(bin, tool), 0o755);
  }
  const result = spawnSync('sh', [SH, ...args], { encoding: 'utf8', env: { PATH: `${bin}:/usr/bin:/bin`, HOME: bin } });
  const tools = existsSync(ran) ? readFileSync(ran, 'utf8').trim().split('\n') : [];
  rmSync(bin, { recursive: true, force: true });
  return { ...result, tools };
}

test('the shell script parses', () => {
  assert.equal(spawnSync('sh', ['-n', SH]).status, 0);
});

test('an unknown or missing agent name is refused before anything is downloaded or opened', () => {
  for (const args of [['nope'], [], ['claude-code; rm -rf ~'], ['../codex']]) {
    const { status, stderr, tools } = runSh(args);
    assert.equal(status, 1, String(args));
    assert.match(stderr, /claude-code, codex, cursor,? (or )?vscode\./);
    assert.deepEqual(tools, [], `nothing ran for ${args}`);
  }
});

test('both scripts name exactly the agents the app can connect', () => {
  const agents = appAgents();
  assert.deepEqual(agents, ['claude-code', 'codex', 'cursor', 'vscode']);
  const shIds = [...readFileSync(SH, 'utf8').matchAll(/^\s+([a-z-]+)\) echo /gm)].map((m) => m[1]).sort();
  const psIds = [...PS1.matchAll(/'([a-z-]+)' = '/g)].map((m) => m[1]).sort();
  assert.deepEqual(shIds, agents);
  assert.deepEqual(psIds, agents);
});

test('the scripts only ask the app to connect, with no token or address in the link', () => {
  for (const script of [readFileSync(SH, 'utf8'), PS1]) {
    const links = [...script.matchAll(/oya:\/\/[^"\s]*/g)].map((m) => m[0]);
    assert.ok(links.length > 0);
    for (const link of links) assert.match(link, /^oya:\/\/agent-connect\?agent=\$[A-Za-z]+$/);
  }
});

test("the shell script checks Oya's signature before it copies the app, and never uses sudo", () => {
  const script = readFileSync(SH, 'utf8');
  assert.ok(script.indexOf('verify_app "$WORK/mount') < script.indexOf('ditto '), 'verified before copied');
  assert.match(script, /TEAM_ID="[A-Z0-9]{10}"/);
  assert.doesNotMatch(code(script), /\bsudo\b/);
  assert.doesNotMatch(code(PS1), /\bsudo\b|RunAs/);
});

test("the PowerShell script stops on an error without closing the person's window", () => {
  assert.doesNotMatch(code(PS1), /\bexit\b/);
  assert.match(PS1, /\$ErrorActionPreference = 'Stop'/);
});

test('the scripts tell the agent it cannot answer the dialog, and what to do if none appears', () => {
  for (const script of [readFileSync(SH, 'utf8'), PS1]) {
    assert.match(script, /an agent cannot answer it/);
    assert.match(script, /If no dialog appears, this copy of Oya is older/);
  }
});
