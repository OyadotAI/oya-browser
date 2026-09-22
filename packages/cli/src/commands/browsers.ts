/**
 * Browser commands: start, goto, ask, ls, rm, status and open.
 */
import { spawn } from 'node:child_process';
import type { BrowserInfo, BrowserDetail, StartOptions, StopResult } from '@oya-ai/browser';
import { flagNum, flagStr, parseJson, type Flags } from '../args.ts';
import { CliError, usage } from '../errors.ts';
import { client, out, targetBrowser } from '../context.ts';
import { LsColumns, StatusColumns } from '../constants.ts';

/** Which browser `oya start` starts: persona, provider and naming flags. */
function identityOptions(flags: Flags): StartOptions {
  return {
    persona: flagStr(flags, 'persona') || 'default',
    provider: flagStr(flags, 'provider') as never,
    wsUrl: flagStr(flags, 'ws-url'),
    name: flagStr(flags, 'name'),
    idempotencyKey: flagStr(flags, 'idempotency-key'),
  };
}

/** How `oya start` is governed: queueing, budget, priority and policy flags. */
function governanceOptions(flags: Flags): StartOptions {
  return {
    queueMs: flagNum(flags, 'queue-ms'),
    budgetUsd: flagNum(flags, 'budget-usd'),
    governed: flags.governed === true,
    priority: flagStr(flags, 'priority') as StartOptions['priority'],
    policy: flagStr(flags, 'policy') ? policyOf(flagStr(flags, 'policy')!) : undefined,
  };
}

/** A --policy value, parsed, or an invalid_json error that shows how to quote it. */
const policyOf = (text: string) =>
  parseJson(text, '--policy', `Quote it: --policy '{"allowedHosts":["example.com"]}'.`) as StartOptions['policy'];

/** `oya start`'s flags as start options. */
const startOptions = (flags: Flags): StartOptions => ({ ...identityOptions(flags), ...governanceOptions(flags) });

/** `oya start`: start a browser and print its id. */
export async function cmdStart(flags: Flags): Promise<void> {
  const browser = await client(flags).browser.start(startOptions(flags));
  const { id, provider, persona, cdpUrl } = browser;
  out(flags, { id, provider, persona, cdpUrl }, () => {
    console.log(`✅ ${id}`);
    console.log(`   provider: ${provider}   persona: ${persona}`);
    if (cdpUrl) console.log(`   cdp:      ${cdpUrl}`);
  });
}

/** `oya goto <url>`: navigate the named or newest browser. */
export async function cmdGoto(args: string[], flags: Flags): Promise<void> {
  const url = args[0];
  if (!url) throw usage('oya goto needs an address: oya goto <url>.');
  const browser = await targetBrowser(client(flags), flags);
  await browser.goto(url);
  out(flags, { id: browser.id, url }, () => console.log(`✅ ${browser.id} → ${url}`));
}

/** `oya ask "<prompt>"`: drive the named or newest browser in plain language. */
export async function cmdAsk(args: string[], flags: Flags): Promise<void> {
  const prompt = args.join(' ');
  if (!prompt) throw usage('oya ask needs a prompt: oya ask "find the pricing page".');
  const browser = await targetBrowser(client(flags), flags);
  const answer = await browser.ask(prompt);
  out(flags, { id: browser.id, answer }, () => console.log(answer));
}

/** A health dot for `oya ls`. */
const DOT: Record<string, string> = { ok: '●', stale: '◐', errors: '✗', dead: '○' };

/** One browser's row in `oya ls`. */
function lsRow(b: BrowserInfo): string {
  const provider = (b.provider || 'oya').padEnd(LsColumns.PROVIDER);
  const persona = (b.personaName || b.persona || 'default').padEnd(LsColumns.PERSONA);
  const url = b.currentUrl.replace(/^https?:\/\//, '').slice(0, LsColumns.URL);
  return `${DOT[b.health] || '·'} ${b.id}  ${provider} ${persona} ${b.name.padEnd(LsColumns.NAME)} ${b.commands}·${b.errors}  ${url}`;
}

/** `oya ls`: what is running. */
export async function cmdLs(flags: Flags): Promise<void> {
  const all = await client(flags).browser.list();
  out(flags, all, () => {
    if (!all.length) return console.log('No browsers running.');
    for (const b of all) console.log(lsRow(b));
    console.log(`\n${all.length} running.`);
  });
}

/** One stopped browser's line in `oya rm`. */
function rmLine(x: StopResult): string {
  const removed = x.sandboxRemoved === true ? ' (sandbox destroyed)' : '';
  const note = x.sandboxRemoved === false ? ', sandbox NOT removed, check Oya Cloud' : removed;
  return `${x.ok ? '✅' : '✗'} ${x.id}${note}${x.error ? ` ${x.error}` : ''}`;
}

/** `oya rm <id>… | --all`: stop browsers. Any that could not be stopped makes it exit 1, after every line is out. */
export async function cmdRm(args: string[], flags: Flags): Promise<void> {
  const oya = client(flags);
  if (!flags.all && !args.length) throw usage('oya rm needs a browser id, or --all: oya rm <id>... | oya rm --all.');
  const r = await oya.browser.stop(flags.all ? 'all' : args);
  out(flags, r, () => printStops(r.results));
  const failed = r.results.filter((x) => !x.ok).length;
  if (failed) throw stopsFailed(r.results.length - failed, r.results.length, !!flags.json);
}

/** Each stop on its own line: the ones done on stdout, the ones refused on stderr, then the count. */
function printStops(results: StopResult[]): void {
  for (const x of results) (x.ok ? console.log : console.error)(rmLine(x));
  console.log(`stopped ${results.filter((x) => x.ok).length} of ${results.length}`);
}

/** A stop that left some browsers running; its lines are already out, so plain mode prints nothing more. */
const stopsFailed = (stopped: number, of: number, json: boolean) =>
  new CliError(`stopped ${stopped} of ${of}`, 'partial_failure', { shown: !json });

/** One recent action's line in `oya status`. */
function activityLine(a: BrowserDetail['activity'][number]): string {
  const error = a.error ? `, ${a.error}` : '';
  return `    ${a.ok ? '·' : '✗'} ${a.action.padEnd(StatusColumns.ACTION)} ${a.summary}${error}  ${a.ms}ms`;
}

/** `oya status`, for people. */
function printStatus(s: BrowserDetail): void {
  console.log(`${s.name}  ${s.id}`);
  console.log(`  ${s.health} · ${s.provider || 'oya'} · persona ${s.personaName || s.persona || 'default'}`);
  console.log(`  ${s.commands} commands · ${s.errors} errors · ${s.pending} in flight · at ${s.currentUrl || '—'}`);
  if (s.lastError) console.log(`  last error: ${s.lastError}`);
  if (!s.activity.length) return;
  console.log('  recent:');
  for (const a of s.activity.slice(0, StatusColumns.RECENT)) console.log(activityLine(a));
}

/** `oya status`: health, counters and what it has been doing. */
export async function cmdStatus(flags: Flags): Promise<void> {
  const browser = await targetBrowser(client(flags), flags);
  const s = await browser.status();
  out(flags, s, () => printStatus(s));
}

/**
 * The platform's opener. `start` is a cmd builtin, not an executable: spawning it
 * directly always fails with ENOENT, and the detached child swallows the error.
 */
function opener(url: string): [string, string[]] {
  if (process.platform === 'darwin') return ['open', [url]];
  if (process.platform === 'win32') return ['cmd', ['/c', 'start', '', url]];
  return ['xdg-open', [url]];
}

/** `oya open`: open the live view in your browser. */
export async function cmdOpen(flags: Flags): Promise<void> {
  const browser = await targetBrowser(client(flags), flags);
  const [command, args] = opener(browser.liveViewUrl());
  spawn(command, args, { detached: true, stdio: 'ignore' }).unref();
  out(flags, { ok: true, id: browser.id }, () => console.log(`Opening the live view of ${browser.id}`));
}
