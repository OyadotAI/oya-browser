/**
 * oya — the command line for the Oya browser control plane.
 *
 *   oya install                stand up a self-hosted control plane
 *   oya login                  save an API key
 *   oya init                   onboarding: model, provider, sign-in
 *   oya start                  start a browser
 *   oya goto <url>             navigate the newest browser
 *   oya ls                     what is running
 *   oya rm <id|--all>          stop browsers
 *   oya personas               identities and their concurrency
 *   oya open                   watch a browser work
 *   oya stealth-test           score this deployment against bot detectors
 */

import { spawn } from 'node:child_process';
import { Oya, OyaError } from '@oya-ai/browser';
import { load, save, resolved, configPath } from './config.js';
import { ask, askSecret, choose, closePrompts } from './prompt.js';
import { cmdInstall } from './install.js';

const HELP = `oya — thousands of browsers, one API

  oya install                     Stand up a self-hosted control plane
            [--dry-run] [--config oya-install.json]
  oya login                       Save an API key for this machine
  oya init                        Set your model, browser provider and sign-ins
  oya start [--persona auto]      Start a browser and print its id
  oya goto <url> [--id <id>]      Navigate (defaults to the newest browser)
  oya ask "<prompt>" [--id <id>]  Drive it in plain language
  oya ls                          List running browsers
  oya rm <id> | --all             Stop browsers
  oya personas                    Identities: fingerprint + cookies + proxy
  oya personas new [name]         --platform Win32|MacIntel|Linux --tz <zone> --locale <l> --max <n>
  oya personas edit <id>          --name <n> --max <n> --geo <cc>
  oya personas clone|rm <id>      A new device of the same kind · delete
  oya status [--id <id>]          Health, counters and what it has been doing
  oya open [--id <id>]            Open the live view in your browser
  oya config [key=value ...]      Show or change this key's settings
  oya control                     Durable project overview
  oya sessions [id]               All sessions, including pending cleanup
  oya stop <id> --force           Stop despite a profile-save error, or reconcile
  oya takeover <id>               Acquire human control
  oya release <id>                Release human control, leaving the agent paused
  oya resume <id>                 Acknowledge agent resume
  oya events [--after <cursor>]   Read durable lifecycle events
  oya project <settings-json>     Update limits, rate cards and retention
  oya start --governed --provider oya-selfhosted [--queue-ms 30000]
            [--budget-usd 5] [--policy JSON] [--idempotency-key ID]
  oya cancel <id>               Cancel queued or provisioning work
  oya recover <id> [--replace]   Explicitly recover or replace a session
  oya members [invite|remove]   List members, invite, or remove a user
  oya credential new [--role viewer|operator|administrator]
  oya credential revoke <id>     Revoke a service credential
  oya webhook new <https-url>    Register a signed event webhook
  oya webhook remove <id>        Disable a webhook
  oya webhook replay <id>        Replay a delivery
  oya usage                       What this key has spent
  oya stealth-test [--live]       Score this deployment against bot detectors

Options: --url <control plane>   --key <api key>   --json
Config:  ${configPath}
`;

type Flags = Record<string, string | boolean>;

function parse(argv: string[]): { command: string; args: string[]; flags: Flags } {
  const [command = 'help', ...rest] = argv;
  const args: string[] = [];
  const flags: Flags = {};
  for (let i = 0; i < rest.length; i++) {
    const token = rest[i];
    if (!token.startsWith('--')) { args.push(token); continue; }
    const name = token.slice(2);
    const next = rest[i + 1];
    if (next === undefined || next.startsWith('--')) flags[name] = true;
    else { flags[name] = next; i++; }
  }
  return { command, args, flags };
}

const flagStr = (flags: Flags, name: string): string | undefined =>
  typeof flags[name] === 'string' ? (flags[name] as string) : undefined;

function client(flags: Flags): Oya {
  const saved = resolved();
  const apiKey = flagStr(flags, 'key') || saved.apiKey;
  const baseUrl = flagStr(flags, 'url') || saved.baseUrl;
  if (!apiKey) {
    console.error('No API key. Run `oya login`, or pass --key / set OYA_API_KEY.');
    process.exit(1);
  }
  return new Oya({ apiKey, baseUrl });
}

const out = (flags: Flags, value: unknown, human: () => void): void => {
  if (flags.json) console.log(JSON.stringify(value, null, 2));
  else human();
};

/** The browser a command acts on when none is named: the most recent one. */
async function targetBrowser(oya: Oya, flags: Flags) {
  const id = flagStr(flags, 'id');
  if (id) return oya.browser.get(id);
  const all = await oya.browser.list();
  if (!all.length) {
    console.error('No browsers running. Start one with `oya start`.');
    process.exit(1);
  }
  return oya.browser.get(all[all.length - 1].id);
}

// ── Commands ────────────────────────────────────────────────────────────────

async function cmdLogin(flags: Flags): Promise<void> {
  const current = resolved();
  // `oya login --key ... --url ...` must not prompt: that is the CI path.
  let apiKey = flagStr(flags, 'key') || '';
  const baseUrl = (flagStr(flags, 'url')
    || (apiKey ? current.baseUrl : await ask('Control plane URL:', current.baseUrl))).replace(/\/+$/, '');

  const how = apiKey ? 'paste' : await choose('How do you want to authenticate?', [
    { id: 'paste', label: 'Paste an API key', note: 'from the dashboard, or a self-hosted key' },
    { id: 'password', label: 'Sign in with email and password', note: 'mints a new key for this machine' },
  ]);

  if (!apiKey && how === 'paste') apiKey = await askSecret('API key:');

  if (!apiKey && how === 'password') {
    const email = await ask('Email:');
    const password = await askSecret('Password:');
    const res = await fetch(`${baseUrl}/api/auth/login`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ email, password }),
    });
    const session = await res.json() as { access_token?: string; error?: string };
    if (!res.ok || !session.access_token) throw new Error(session.error || 'Sign-in failed');

    const minted = await fetch(`${baseUrl}/api/auth/keys`, {
      method: 'POST',
      headers: { Authorization: `Bearer ${session.access_token}`, 'Content-Type': 'application/json' },
      body: JSON.stringify({ label: 'CLI' }),
    });
    const created = await minted.json() as { key?: string; error?: string };
    if (!minted.ok || !created.key) throw new Error(created.error || 'Could not create an API key');
    apiKey = created.key;
    console.log('  Created a new API key labelled "CLI".');
  }

  if (!apiKey) throw new Error('No API key given');

  // Prove it works before writing it — a saved-but-wrong key is a bad first run.
  const check = await fetch(`${baseUrl}/api/config`, { headers: { Authorization: `Bearer ${apiKey}` } });
  if (!check.ok) throw new Error(`That key was rejected by ${baseUrl} (${check.status})`);

  save({ apiKey, baseUrl });
  console.log(`\n✅ Signed in to ${baseUrl}. Saved to ${configPath}`);
  console.log('   Next: `oya init` to pick your model and browser provider.');
}

async function cmdInit(flags: Flags): Promise<void> {
  const oya = client(flags);
  const current = await oya.config.get<{
    providers: Array<{ id: string; label: string; configured: boolean; needs: string[] }>;
    has_openai_key: boolean;
    chat_model: string;
  }>();

  console.log('\n── 1. Your model ──');
  const llm = await choose('Which LLM should agents use?', [
    { id: 'anthropic', label: 'Claude (Anthropic)' },
    { id: 'openai', label: 'OpenAI' },
    { id: 'gemini', label: 'Gemini (Google)' },
    { id: 'vertex', label: 'Gemini Enterprise (Vertex AI)' },
    { id: 'skip', label: 'Skip', note: current.has_openai_key ? 'keep what is configured' : 'no agent control' },
  ]);

  const presets: Record<string, { name: string; model: string }> = {
    anthropic: { name: 'Anthropic', model: 'claude-sonnet-4-5' },
    openai: { name: 'OpenAI', model: 'gpt-4o-mini' },
    gemini: { name: 'Gemini', model: 'gemini-3.8-flash' },
    vertex: { name: 'Gemini Enterprise', model: 'gemini-2.5-flash' },
  };
  const updates: Record<string, unknown> = {};
  if (llm !== 'skip') {
    updates.llm_provider = llm;
    const key = await askSecret(`${presets[llm].name} API key:`);
    if (key) updates.openai_api_key = key;
    const model = await ask('Default model:', presets[llm].model);
    if (model) updates.chat_model = model;
  }

  console.log('\n── 2. Where your browsers run ──');
  const provider = await choose('Browser provider:', current.providers.map((p) => ({
    id: p.id,
    label: p.label,
    note: p.needs.length ? (p.configured ? 'configured' : 'needs an API key') : undefined,
  })));
  updates.browser_provider = provider;

  const needs = current.providers.find((p) => p.id === provider)?.needs || [];
  for (const field of needs) {
    const value = await askSecret(`${field.replace(/_/g, ' ')}:`);
    if (value) updates[field] = value;
  }

  console.log('\n── 3. CAPTCHAs ──');
  const solver = await choose('Solve CAPTCHAs automatically?', [
    { id: '', label: 'No', note: 'providers that solve natively still will' },
    { id: 'capsolver', label: 'Yes, via CapSolver' },
  ]);
  updates.captcha_solver = solver;
  if (solver) {
    const key = await askSecret('Solver API key:');
    if (key) updates.captcha_api_key = key;
  }

  updates.onboarded = 'true';
  await oya.config.set(updates);
  console.log('\n✅ Saved against your API key.');

  if (provider === 'oya-cloud' || provider === 'oya-selfhosted') {
    console.log('\n── 4. Sign in once, on your own machine ──');
    console.log('   Your remote browsers reuse the cookies from a desktop sign-in, so agents');
    console.log('   arrive already logged in — as the same identity, from the same fingerprint.');
    console.log(`   Download the desktop browser: ${resolved().baseUrl}/downloads`);
  }

  console.log('\n   Then:  oya start && oya goto https://example.com');
}

async function cmdStart(flags: Flags): Promise<void> {
  const oya = client(flags);
  const browser = await oya.browser.start({
    persona: flagStr(flags, 'persona') || 'default',
    captcha: flags.captcha === false ? 'off' : 'auto',
    provider: flagStr(flags, 'provider') as never,
    wsUrl: flagStr(flags, 'ws-url'),
    name: flagStr(flags, 'name'),
    idempotencyKey: flagStr(flags, 'idempotency-key'),
    queueMs: flagStr(flags, 'queue-ms') === undefined ? undefined : Number(flagStr(flags, 'queue-ms')),
    budgetUsd: flagStr(flags, 'budget-usd') === undefined ? undefined : Number(flagStr(flags, 'budget-usd')),
    governed: flags.governed === true,
    priority: flagStr(flags, 'priority') as 'low' | 'normal' | 'high' | undefined,
    policy: flagStr(flags, 'policy') ? JSON.parse(flagStr(flags, 'policy')!) : undefined,
  });
  out(flags, { id: browser.id, provider: browser.provider, persona: browser.persona, cdpUrl: browser.cdpUrl }, () => {
    console.log(`✅ ${browser.id}`);
    console.log(`   provider: ${browser.provider}   persona: ${browser.persona}`);
    if (browser.cdpUrl) console.log(`   cdp:      ${browser.cdpUrl}`);
  });
}

async function cmdGoto(args: string[], flags: Flags): Promise<void> {
  const url = args[0];
  if (!url) throw new Error('Usage: oya goto <url>');
  const oya = client(flags);
  const browser = await targetBrowser(oya, flags);
  await browser.goto(url);
  console.log(`✅ ${browser.id} → ${url}`);
}

async function cmdAsk(args: string[], flags: Flags): Promise<void> {
  const prompt = args.join(' ');
  if (!prompt) throw new Error('Usage: oya ask "find the pricing page"');
  const oya = client(flags);
  const browser = await targetBrowser(oya, flags);
  console.log(await browser.ask(prompt));
}

async function cmdLs(flags: Flags): Promise<void> {
  const all = await client(flags).browser.list();
  out(flags, all, () => {
    if (!all.length) return console.log('No browsers running.');
    const dot: Record<string, string> = { ok: '●', stale: '◐', errors: '✗', dead: '○' };
    for (const b of all) {
      console.log(`${dot[b.health] || '·'} ${b.id}  ${(b.provider || 'oya').padEnd(14)} ${(b.personaName || b.persona || 'default').padEnd(14)} ${b.name.padEnd(18)} ${b.commands}·${b.errors}  ${b.currentUrl.replace(/^https?:\/\//, '').slice(0, 40)}`);
    }
    console.log(`\n${all.length} running.`);
  });
}

async function cmdRm(args: string[], flags: Flags): Promise<void> {
  const oya = client(flags);
  if (!flags.all && !args.length) throw new Error('Usage: oya rm <id>… | oya rm --all');
  const r = await oya.browser.stop(flags.all ? 'all' : args);
  for (const x of r.results) {
    const note = x.sandboxRemoved === true ? ' (sandbox destroyed)' : x.sandboxRemoved === false ? ' — sandbox NOT removed, check Oya Cloud' : '';
    console.log(`${x.ok ? '✅' : '✗'} ${x.id}${note}${x.error ? ` ${x.error}` : ''}`);
  }
  console.log(`stopped ${r.stopped}`);
}

async function cmdStatus(flags: Flags): Promise<void> {
  const oya = client(flags);
  const browser = await targetBrowser(oya, flags);
  const s = await browser.status();
  out(flags, s, () => {
    console.log(`${s.name}  ${s.id}`);
    console.log(`  ${s.health} · ${s.provider || 'oya'} · persona ${s.personaName || s.persona || 'default'}`);
    console.log(`  ${s.commands} commands · ${s.errors} errors · ${s.pending} in flight · at ${s.currentUrl || '—'}`);
    if (s.lastError) console.log(`  last error: ${s.lastError}`);
    if (s.activity.length) {
      console.log('  recent:');
      for (const a of s.activity.slice(0, 10)) console.log(`    ${a.ok ? '·' : '✗'} ${a.action.padEnd(18)} ${a.summary}${a.error ? ` — ${a.error}` : ''}  ${a.ms}ms`);
    }
  });
}

async function cmdPersonas(args: string[], flags: Flags): Promise<void> {
  const oya = client(flags);
  const [sub, ...rest] = args;

  if (sub === 'new' || sub === 'create') {
    const platformAlias: Record<string, 'Win32' | 'MacIntel' | 'Linux x86_64'> = {
      win32: 'Win32', windows: 'Win32', win: 'Win32', macintel: 'MacIntel', mac: 'MacIntel', macos: 'MacIntel',
      linux: 'Linux x86_64', 'linux x86_64': 'Linux x86_64',
    };
    const platformFlag = flagStr(flags, 'platform');
    const prefs = {
      ...(platformFlag ? { platform: platformAlias[platformFlag.toLowerCase()] || (platformFlag as never) } : {}),
      ...(flagStr(flags, 'tz') ? { timezone: flagStr(flags, 'tz') } : {}),
      ...(flagStr(flags, 'locale') ? { locale: flagStr(flags, 'locale') } : {}),
    };
    if (flags.preview) {
      const fp = await oya.personas.preview(prefs);
      return out(flags, fp, () => console.log(`${fp.platform} · ${fp.timezone} · ${fp.locale} · ${fp.screen} · ${fp.webgl}`));
    }
    const created = await oya.personas.create({
      name: flagStr(flags, 'name') || rest[0],
      prefs,
      proxy: flagStr(flags, 'geo') ? { geo: flagStr(flags, 'geo') } : undefined,
      maxConcurrent: flagStr(flags, 'max') ? Number(flagStr(flags, 'max')) : undefined,
    });
    return out(flags, created, () => console.log(`✅ ${created.id}  ${created.name}  ${created.fingerprint.platform} · ${created.fingerprint.timezone}`));
  }

  if (sub === 'edit') {
    if (!rest[0]) throw new Error('Usage: oya personas edit <id> --name <n> --max <n> --geo <cc>');
    const updated = await oya.personas.update(rest[0], {
      ...(flagStr(flags, 'name') ? { name: flagStr(flags, 'name') } : {}),
      ...(flagStr(flags, 'max') ? { maxConcurrent: flagStr(flags, 'max') === 'none' ? null : Number(flagStr(flags, 'max')) } : {}),
      ...(flagStr(flags, 'geo') ? { proxy: { geo: flagStr(flags, 'geo') } } : {}),
    });
    return out(flags, updated, () => console.log(`✅ ${updated.id}  ${updated.name}  cap ${updated.maxConcurrent ?? '∞'}`));
  }

  if (sub === 'clone') {
    if (!rest[0]) throw new Error('Usage: oya personas clone <id> [--name <n>]');
    const c = await oya.personas.clone(rest[0], { name: flagStr(flags, 'name') });
    return out(flags, c, () => console.log(`✅ ${c.id}  ${c.name}  ${c.fingerprint.platform} · ${c.fingerprint.timezone}  (new identity, same kind of device)`));
  }

  if (sub === 'rm' || sub === 'delete') {
    if (!rest[0]) throw new Error('Usage: oya personas rm <id>');
    await oya.personas.remove(rest[0]);
    return console.log(`✅ removed ${rest[0]}`);
  }

  const all = await oya.personas.list();
  out(flags, all, () => {
    if (!all.length) return console.log('No personas yet — `oya personas new`.');
    for (const p of all) {
      const cap = p.maxConcurrent === null ? '∞' : String(p.maxConcurrent);
      console.log(`${p.id}  ${p.name.padEnd(20)} ${p.activeBrowsers}/${cap} running  ${p.fingerprint.platform} · ${p.fingerprint.timezone}`
        + `${p.exit ? `  via ${p.exit.label}` : p.proxy?.geo ? `  geo ${p.proxy.geo}` : ''}${p.mfa?.configured ? `  mfa:${p.mfa.type}` : ''}${p.isDefault ? '  (default)' : ''}`);
    }
  });
}

async function cmdOpen(flags: Flags): Promise<void> {
  const browser = await targetBrowser(client(flags), flags);
  const url = browser.liveViewUrl();
  // `start` is a cmd builtin, not an executable: spawning it directly always
  // fails with ENOENT, and the detached child swallows the error.
  const [opener, args] = process.platform === 'darwin' ? ['open', [url]]
    : process.platform === 'win32' ? ['cmd', ['/c', 'start', '', url]]
    : ['xdg-open', [url]];
  spawn(opener, args, { detached: true, stdio: 'ignore' }).unref();
  console.log(`Opening ${browser.id}`);
}

async function cmdConfig(args: string[], flags: Flags): Promise<void> {
  const oya = client(flags);
  if (!args.length) {
    const current = await oya.config.get();
    return out(flags, current, () => console.log(JSON.stringify(current, null, 2)));
  }
  const updates: Record<string, string> = {};
  for (const pair of args) {
    const index = pair.indexOf('=');
    if (index < 1) throw new Error(`Expected key=value, got "${pair}"`);
    updates[pair.slice(0, index)] = pair.slice(index + 1);
  }
  await oya.config.set(updates);
  console.log(`✅ updated ${Object.keys(updates).join(', ')}`);
}

async function cmdUsage(flags: Flags): Promise<void> {
  const usage = await client(flags).usage();
  console.log(JSON.stringify(usage, null, 2));
}

/**
 * The stealth harness lives with the server, because it needs a browser to
 * measure. This just runs it where it is.
 */
function cmdStealthTest(flags: Flags): Promise<void> {
  return new Promise((resolve, reject) => {
    const child = spawn('node', ['test-stealth.js', ...(flags.live ? ['--live'] : [])], {
      cwd: process.env.OYA_SERVER_DIR || 'server',
      stdio: 'inherit',
    });
    child.on('error', () => reject(new Error(
      'Could not find the stealth harness. Run it from a checkout, or set OYA_SERVER_DIR.')));
    child.on('exit', (code) => (code === 0 ? resolve() : reject(new Error(`stealth test exited ${code}`))));
  });
}

// ── Entry ───────────────────────────────────────────────────────────────────

const { command, args, flags } = parse(process.argv.slice(2));

try {
  // Plain `oya stop <id...>|--all` keeps its bulk meaning; only --force needs the durable endpoint.
  if (['control', 'sessions', 'takeover', 'release', 'resume', 'events', 'project', 'credential', 'webhook', 'cancel', 'recover', 'members'].includes(command) || (command === 'stop' && flags.force === true)) {
    const c = client(flags).control;
    const required = (i = 0) => { if (!args[i]) throw new Error('Missing argument; run oya help'); return args[i]; };
    let result: unknown;
    switch (command) {
      case 'control': result = await c.overview(); break;
      case 'sessions': result = args[0] ? await c.session(args[0]) : await c.sessions(); break;
      case 'cancel': result = await c.cancel(required()); break;
      case 'recover': result = await c.recover(required(), flags.replace === true); break;
      case 'members':
        if (!args[0]) result = await c.members();
        else if (args[0] === 'remove') result = await c.removeMember(required(1));
        else if (args[0] === 'invite') {
          const role = flagStr(flags, 'role') || 'operator';
          if (!['viewer', 'operator', 'administrator'].includes(role)) throw new Error('Invalid role');
          result = await c.inviteMember(role as 'viewer' | 'operator' | 'administrator');
        } else throw new Error('Use members, members invite, or members remove');
        break;
      case 'stop': result = await c.stop(required(), flags.force === true); break;
      case 'takeover': case 'release': case 'resume': result = await c.takeover(required(), command === 'takeover' ? 'acquire' : command); break;
      case 'events': result = await c.events(Number(flagStr(flags, 'after') || 0)); break;
      case 'project': result = await c.settings(JSON.parse(required())); break;
      case 'credential':
        if (required() === 'revoke') result = await c.revokeCredential(required(1));
        else if (args[0] === 'new') {
          const role = flagStr(flags, 'role') || 'operator';
          if (!['viewer', 'operator', 'administrator'].includes(role)) throw new Error('Invalid role');
          result = await c.createCredential({ role: role as 'viewer' | 'operator' | 'administrator', label: flagStr(flags, 'label') });
        } else throw new Error('Use credential new or credential revoke');
        break;
      case 'webhook':
        if (required() === 'new') result = await c.createWebhook(required(1));
        else if (args[0] === 'remove') result = await c.removeWebhook(required(1));
        else if (args[0] === 'replay') result = await c.replayDelivery(required(1));
        else throw new Error('Use webhook new, remove, or replay');
        break;
    }
    console.log(JSON.stringify(result, null, 2));
  } else switch (command) {
    case 'install':      await cmdInstall(flags); break;
    case 'login':        await cmdLogin(flags); break;
    case 'init':         await cmdInit(flags); break;
    case 'start':        await cmdStart(flags); break;
    case 'goto':         await cmdGoto(args, flags); break;
    case 'ask':          await cmdAsk(args, flags); break;
    case 'ls': case 'list': await cmdLs(flags); break;
    case 'status':       await cmdStatus(flags); break;
    case 'rm': case 'stop': await cmdRm(args, flags); break;
    case 'personas':     await cmdPersonas(args, flags); break;
    case 'open':         await cmdOpen(flags); break;
    case 'config':       await cmdConfig(args, flags); break;
    case 'usage':        await cmdUsage(flags); break;
    case 'stealth-test': await cmdStealthTest(flags); break;
    case 'whoami':       console.log(JSON.stringify({ ...resolved(), apiKey: load().apiKey ? 'saved' : 'none' }, null, 2)); break;
    case 'help': case '--help': case '-h': console.log(HELP); break;
    default:
      console.error(`Unknown command: ${command}\n`);
      console.log(HELP);
      process.exit(1);
  }
} catch (err) {
  const error = err as OyaError;
  console.error(`\n✗ ${error.message}`);
  if (error.status === 401) console.error('  The API key was rejected. Run `oya login`.');
  if (error.status === 429) console.error('  A quota or a persona concurrency cap. `oya personas` shows what is running.');
  process.exit(1);
} finally {
  // An open readline holds the event loop open, so the process would never exit.
  closePrompts();
}
