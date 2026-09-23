/**
 * A fake command-line tool on PATH, for code that shells out to docker or
 * kubectl. Stubbing the module would skip the real execFile/spawn path; a fake
 * binary runs it, and records exactly the argv and stdin the tool received,
 * so a test can prove a credential never reached argv.
 */
import { after } from 'node:test';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { restoreEnv } from './data-dir.ts';

/** One call the fake received. */
export type FakeCall = { args: string[]; stdin: string; files: Record<string, string> };

/**
 * Puts a fake `name` on PATH that logs each call and then runs `body`, a Node
 * script fragment with `args` (argv) and `reply(out)` / `fail(err)` in scope.
 * The contents of any file named after `--env-file` are logged too, since the
 * real caller deletes that file as soon as the tool returns.
 */
export function fakeCli(name: string, body: string) {
  const dir = mkdtempSync(join(tmpdir(), `oya-fake-${name}-`));
  const log = join(dir, 'calls.jsonl');
  mkdirSync(join(dir, 'bin'));
  writeFileSync(join(dir, 'bin', name), script(log, body), { mode: 0o755 });
  const savedPath = process.env.PATH;
  process.env.PATH = `${join(dir, 'bin')}:${savedPath}`;
  after(() => {
    restoreEnv('PATH', savedPath);
    rmSync(dir, { recursive: true, force: true });
  });
  return { calls: () => readCalls(log), reset: () => rmSync(log, { force: true }) };
}

/** The calls logged so far. */
function readCalls(log: string): FakeCall[] {
  if (!existsSync(log)) return [];
  return readFileSync(log, 'utf8')
    .trim()
    .split('\n')
    .filter(Boolean)
    .map((line) => JSON.parse(line));
}

/** The fake's source: log the call, then run the body. */
const script = (log: string, body: string) => `#!/usr/bin/env node
const fs = require('node:fs');
const args = process.argv.slice(2);
let stdin = '';
// Only '-f -' writes stdin; execFile leaves it open, so reading otherwise would hang.
if (args.includes('-')) stdin = fs.readFileSync(0, 'utf8');
const files = {};
const at = args.indexOf('--env-file');
if (at >= 0) files[args[at + 1]] = fs.readFileSync(args[at + 1], 'utf8');
fs.appendFileSync(${JSON.stringify(log)}, JSON.stringify({ args, stdin, files }) + '\\n');
const reply = (out) => { process.stdout.write(typeof out === 'string' ? out : JSON.stringify(out)); process.exit(0); };
const fail = (err) => { process.stderr.write(err); process.exit(1); };
${body}
`;
