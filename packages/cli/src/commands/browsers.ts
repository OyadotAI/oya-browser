/**
 * Browser commands: start, goto, ask, ls, rm, status and open.
 */
import { spawn } from 'node:child_process';
import type { BrowserInfo, BrowserDetail, StartOptions, StopResult } from '@oya-ai/browser';
import { flagNum, flagStr, type Flags } from '../args.ts';
import { client, out, targetBrowser } from '../context.ts';
import { LsColumns, StatusColumns } from '../constants.ts';

/** Which browser `oya start` starts: persona, provider and naming flags. */
function identityOptions(flags: Flags): StartOptions {
  return {
    persona: flagStr(flags, 'persona') || 'default',
    captcha: flags.captcha === false ? 'off' : 'auto',
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
    policy: flagStr(flags, 'policy') ? JSON.parse(flagStr(flags, 'policy')!) : undefined,
  };
}

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
  if (!url) throw new Error('Usage: oya goto <url>');
  const browser = await targetBrowser(client(flags), flags);
  await browser.goto(url);
  console.log(`✅ ${browser.id} → ${url}`);
}

/** `oya ask "<prompt>"`: drive the named or newest browser in plain language. */
export async function cmdAsk(args: string[], flags: Flags): Promise<void> {
  const prompt = args.join(' ');
  if (!prompt) throw new Error('Usage: oya ask "find the pricing page"');
  const browser = await targetBrowser(client(flags), flags);
  console.log(await browser.ask(prompt));
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
  const note = x.sandboxRemoved === false ? ' — sandbox NOT removed, check Oya Cloud' : removed;
  return `${x.ok ? '✅' : '✗'} ${x.id}${note}${x.error ? ` ${x.error}` : ''}`;
}

/** `oya rm <id>… | --all`: stop browsers. */
export async function cmdRm(args: string[], flags: Flags): Promise<void> {
  const oya = client(flags);
  if (!flags.all && !args.length) throw new Error('Usage: oya rm <id>… | oya rm --all');
  const r = await oya.browser.stop(flags.all ? 'all' : args);
  for (const x of r.results) console.log(rmLine(x));
  console.log(`stopped ${r.stopped}`);
}

/** One recent action's line in `oya status`. */
function activityLine(a: BrowserDetail['activity'][number]): string {
  const error = a.error ? ` — ${a.error}` : '';
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
  console.log(`Opening ${browser.id}`);
}
