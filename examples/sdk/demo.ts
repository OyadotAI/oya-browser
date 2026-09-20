/**
 * Oya Browser: a live walkthrough to run in front of a customer.
 *
 *   npx tsx --env-file=.env sdk/demo.ts              # press Enter between steps
 *   npx tsx --env-file=.env sdk/demo.ts --no-pause   # run straight through
 *
 * Every step runs on your account. A step that needs something your key
 * hasn't configured (a vendor key, a CAPTCHA solver, an LLM) says what's
 * missing and the demo moves on.
 */
import { mkdir, writeFile } from 'node:fs/promises';
import { createInterface } from 'node:readline/promises';
import { Oya, type Provider } from '@oya-ai/browser';

const oya = new Oya();
const console_ = process.env.OYA_BASE_URL || 'https://browser.getoya.ai';
const rl = process.stdin.isTTY && !process.argv.includes('--no-pause')
  ? createInterface({ input: process.stdin, output: process.stdout }) : null;

const paint = (code: number) => (s: string) => `\x1b[${code}m${s}\x1b[0m`;
const [bold, dim, green, yellow, red] = [1, 2, 32, 33, 31].map(paint);
const say = (s = '') => console.log(`   ${s}`);
const title = async (b: { tabs(): Promise<{ active: boolean; title: string }[]> }) => (await b.tabs()).find((t) => t.active)?.title ?? '';

let n = 0;
/** One demo step: numbered, timed, and never fatal. */
async function step<T>(name: string, run: () => Promise<T>): Promise<T | undefined> {
  if (rl && n) await rl.question(dim('\n   ↵ '));
  console.log(`\n${bold(`${++n}. ${name}`)}`);
  const started = Date.now();
  try {
    const value = await run();
    say(dim(`${((Date.now() - started) / 1000).toFixed(1)}s`));
    return value;
  } catch (e) {
    say(red(`✗ ${(e as Error).message.split('\n')[0].replace(/[\s—{-]+$/, '')}`));
  }
}

type Config = { providers: { id: Provider; label: string; configured: boolean }[]; effective?: { hasLlmKey?: boolean } };
const config = await oya.config.get<Config>();
const vendors = config.providers.filter((p) => p.configured && !['cdp', 'oya-selfhosted'].includes(p.id)).map((p) => p.id);

console.log(bold('\nOya Browser: the control plane for AI browser agents'));

await step('One API over every browser vendor', async () => {
  for (const p of config.providers) {
    say(`${p.configured ? green('●') : dim('○')} ${p.label.padEnd(28)} ${p.configured ? 'ready' : dim('add its key in the dashboard')}`);
  }
});

const persona = await step('A persona: one stable device identity', async () => {
  const p = (await oya.personas.list()).find((x) => x.name === 'demo-shopper')
    ?? await oya.personas.create({ name: 'demo-shopper', prefs: { platform: 'MacIntel', timezone: 'America/New_York', locale: 'en-US' } });
  const f = p.fingerprint;
  say(`${p.name}: ${f.platform} · ${f.webgl} · ${f.hardwareConcurrency} cores · ${f.screen} · ${f.timezone} · ${f.locale}`);
  say(dim('Same fingerprint, cookie jar and proxy on every run, so sites see a returning device.'));
  return p;
});

await using browser = await step('Start a cloud browser as that persona', async () => {
  const b = await oya.browser.start({ persona: persona?.id, provider: 'oya-cloud' });
  say(`${b.id} on ${b.provider}`);
  say(`Watch it live: ${console_} → Browsers`);
  return b;
});

await step('Read a real Amazon product page', async () => {
  await browser!.goto('https://www.amazon.com/dp/B09B8V1LZ3');
  const { markdown, elements } = await browser!.analyze();
  if (/Enter the characters you see below/i.test(markdown)) {
    return say(yellow('Amazon asked this IP for a CAPTCHA. In production that goes to your solver, or to a person in the live view.'));
  }
  const text = (...ids: string[]) => elements.find((e) => e.domId && ids.includes(e.domId))?.text?.trim() ?? '';
  const price = text('corePriceDisplay_desktop_feature_div', 'apex_offerDisplay_desktop', 'corePrice_feature_div').match(/\$[\d,.]+/)?.[0];
  say(await title(browser!));
  say(`${price ?? 'price hidden'} · ${text('availabilityInsideBuyBox_feature_div') || 'stock n/a'} · ${elements.length} page elements mapped for the agent`);
});

await step('Detect and solve a CAPTCHA', async () => {
  await browser!.goto('https://www.google.com/recaptcha/api2/demo');
  const r = await browser!.solveCaptcha();
  say(r.present ? `${r.type} detected` : 'no CAPTCHA on the page');
  if (r.solved) say(green(`solved by ${r.method === 'provider' ? 'the browser vendor' : 'your CAPTCHA solver'}`));
  else if (r.present) say(yellow('choose a CAPTCHA solver in the dashboard settings to solve it automatically'));
});

await step('Take a screenshot', async () => {
  const [, type, data] = (await browser!.screenshot()).match(/^data:image\/(\w+);base64,(.+)$/s) ?? [];
  const file = `out/demo.${type === 'jpeg' ? 'jpg' : type}`;
  await mkdir('out', { recursive: true });
  await writeFile(file, Buffer.from(data, 'base64'));
  say(file);
});

await step('Drive it in plain English', async () => {
  if (!config.effective?.hasLlmKey) return say(yellow('add an OpenAI or Anthropic key in the dashboard to enable this'));
  await browser!.goto('https://news.ycombinator.com');
  say(await browser!.ask('What are the top 3 stories and how many points does each have? One line each.'));
});

await step(`A fleet: three browsers at once on ${vendors.join(', ')}`, async () => {
  const sites = ['https://news.ycombinator.com', 'https://github.com/trending', 'https://en.wikipedia.org/wiki/Main_Page'];
  const results = await Promise.allSettled(sites.map(async (url, i) => {
    const provider = vendors[i % vendors.length];
    await using b = await oya.browser.start({ provider });
    await b.goto(url);
    return `${provider.padEnd(12)} ${await title(b)}`;
  }));
  results.forEach((r) => say(r.status === 'fulfilled' ? r.value : red(`✗ ${r.reason.message}`)));
  if (vendors.length === 1) say(dim('Add Browserbase, Steel, Anchor or Browser Use keys and the same code spreads across them.'));
});

await step('Two-factor login', async () => {
  if (!process.env.TOTP_SECRET || !process.env.MFA_URL) {
    return say(yellow('set TOTP_SECRET and MFA_URL in .env to run this live. The seed is sealed on the persona and never returned.'));
  }
  await oya.personas.setMfa(persona!.id, { type: 'totp', secret: process.env.TOTP_SECRET });
  await browser!.goto(process.env.MFA_URL);
  if (process.env.MFA_USER) { // a login form that asks for the code alongside the password
    const els = (await browser!.analyze()).elements;
    const field = (re: RegExp) => els.find((e) => e.type === 'input' && re.test(`${e.name} ${e.placeholder}`));
    await browser!.type(field(/mail|user/i)!.id, process.env.MFA_USER);
    await browser!.type(field(/pass/i)!.id, process.env.MFA_PASSWORD ?? '');
  }
  const mfa = await browser!.completeMfa();
  if (mfa.completed) say(green(`code entered with ${mfa.method}`));
  else if (!mfa.present) say(yellow('no code field recognized on this page'));
  else say(yellow(`needs a person: ${mfa.liveViewUrl ?? mfa.error}`));
  say(`now on: ${await title(browser!)}`);
});

console.log(bold('\nDone.') + dim(' Every browser is stopped and its cloud sandbox destroyed.\n'));
rl?.close();
