# @oya-ai/browser

<p align="center">
  <strong>A real browser your agents drive from the inside. Record a run once, replay it with no model in the loop.</strong>
</p>

<p align="center">
  <a href="https://www.npmjs.com/package/@oya-ai/browser"><img src="https://img.shields.io/npm/v/@oya-ai/browser?color=39ed35&label=@oya-ai/browser&logo=npm" alt="NPM Version"></a>
  <a href="https://github.com/OyadotAI/oya-browser/blob/main/packages/sdk/LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg?color=39ed35" alt="License: MIT"></a>
  <a href="https://www.typescriptlang.org"><img src="https://img.shields.io/badge/TypeScript-Ready-3178C6.svg?logo=typescript&logoColor=white" alt="TypeScript"></a>
  <a href="https://nodejs.org"><img src="https://img.shields.io/badge/node-%3E%3D20.0.0-brightgreen.svg" alt="Node Version"></a>
  <a href="https://bundlephobia.com/package/@oya-ai/browser"><img src="https://img.shields.io/bundlephobia/minzip/@oya-ai/browser?color=39ed35" alt="Bundle Size"></a>
</p>

---

## From nothing to an answer

```bash
npm install @oya-ai/browser
```

```js
import { Oya } from '@oya-ai/browser';

const browser = await new Oya().browser.start();
await browser.goto('https://news.ycombinator.com');
console.log(await browser.ask('What are the top 3 stories?'));
```

Two things to have first: an API key from [oyabrowser.com](https://oyabrowser.com)
(or your own deployment) in `OYA_API_KEY`, and a browser to drive.

**The fastest browser to have is one you already have.** Open the
[Oya desktop browser](https://oyabrowser.com) and sign in with the same key: `start()`
hands over the browser that is already connected when no provider is configured, so the
five lines above work with nothing else set up. `stop()` leaves a browser you borrowed
alone, because you did not start it.

When you would rather it start one for you, run `npx @oya-ai/cli init` and pick where
browsers run: Oya Cloud, your own Docker, Browserbase, Steel, Anchor, Browser Use, or a
Chrome of your own over CDP. Your code does not change.

**Where the key comes from**, in order: `new Oya({ apiKey })`, then `OYA_API_KEY`, then the
file `oya login` wrote (`~/.oya/config.json`, or `OYA_CONFIG_HOME`), which is read on Node
22.3 and newer. `baseUrl` resolves the same way. A CI job that sets neither fails loudly
rather than borrowing whatever is on the machine.

Node.js 20 or newer; the examples are ES modules. `OYA_BASE_URL` points at a self-hosted
control plane. On Node.js 24+, `await using browser = await oya.browser.start()` stops it
when the scope exits, errors included.

## Portal automation: record once, replay with new inputs

This example adapts the portal-automation project's workflow: reuse a persona, attach to an existing browser or start one, run a prompt the first time, then replay its saved playbook. The portal, workflow names, and request values below are fictional. Supply your own test portal and credentials through environment variables; adapt the task to its actual pages.

Save as `portal.mjs` and run `node portal.mjs` after setting `OYA_API_KEY`, `PORTAL_URL`, `PORTAL_USERNAME`, and `PORTAL_PASSWORD`. Set `OYA_BROWSER_ID` only to reuse an already running browser.

```js
import { createInterface } from 'node:readline/promises';
import { Oya } from '@oya-ai/browser';

function requiredEnv(name) {
  const value = process.env[name];
  if (!value) throw new Error(`Set ${name} before running this example.`);
  return value;
}

const oya = new Oya({ apiKey: requiredEnv('OYA_API_KEY') });
const portalUrl = requiredEnv('PORTAL_URL');
const playbookName = 'portal-request-review';
const secrets = {
  username: requiredEnv('PORTAL_USERNAME'),
  password: requiredEnv('PORTAL_PASSWORD'),
};
// Fictional test inputs. data is visible to the agent.
const data = {
  customerName: 'Alex Example',
  requestId: 'DEMO-0001',
  requestedDate: '2030-01-15',
};
const task = [
  'If not logged in, log in with {{username}} and {{password}}.',
  'Open New Request and enter {{requestId}} as the reference.',
  'Fill first name {{customerName|first}} and last name {{customerName|last}}.',
  'Set the requested date to {{requestedDate|date:MM/DD/YYYY}}.',
  'If a field is already correct, do not type its value again.',
  'If an action times out, inspect the page before retrying it.',
  'If information is missing, ask the person instead of guessing.',
  'Stop on the review page. Do not submit the request.',
].join('\n');

const existingId = process.env.OYA_BROWSER_ID;
let browser;
if (existingId) {
  browser = await oya.browser.get(existingId);
} else {
  const persona =
    (await oya.personas.list()).find((p) => p.name === 'portal-demo') ??
    (await oya.personas.create({ name: 'portal-demo' }));
  browser = await oya.browser.start({ persona: persona.id, captcha: 'auto' });
}

try {
  console.log('Watch in your Oya dashboard:', browser.liveViewUrl());
  await browser.goto(portalUrl);
  const exists = (await oya.playbooks.list()).some((p) => p.name === playbookName);
  const run = await browser.submit(exists ? { playbook: playbookName } : { prompt: task }, {
    // Replay accepts all variables in data; the playbook remembers secret names.
    ...(exists ? { data: { ...data, ...secrets } } : { data, secrets }),
    onSuccess: () => console.log('Run succeeded.'),
    onFailure: (error) => console.error('Run failed with status:', error.status),
    onHealed: (result) => {
      console.log('A repair draft is ready for review:', result.draft);
    },
    onHumanAttention: async (request) => {
      console.log('Attention needed:', request.reason);
      console.log(request.message);
      console.log('Open:', request.liveViewUrl ?? browser.liveViewUrl());
      const terminal = createInterface({ input: process.stdin, output: process.stdout });
      try {
        const answer = await terminal.question(
          request.reason === 'agent' ? 'Answer the agent: ' : 'Handle this in the live view, then press Enter: ',
        );
        await request.respond(answer || 'done');
      } finally {
        terminal.close();
      }
    },
  });

  await run.done; // Rejects on failure; do not save a failed run as a playbook.
  const info = await run.status();
  console.log('Run status:', info.status);
  if (!exists) {
    const playbook = await browser.toPlaybook(playbookName);
    console.log('Saved:', playbook.name, 'Steps:', playbook.steps);
    console.log('Variables:', playbook.variables);
    // playbook.code contains the flow as an exported Playwright module.
  }
} finally {
  // Leave an attached browser open; stop only the browser this script started.
  if (!existingId) await browser.stop();
}
```

`submit()` starts a background run and returns a `Run` handle. The SDK polls every two seconds by default (`pollMs` overrides this). `run.done` resolves with the result or rejects with an error; `run.status()` reads the run record. Attention reasons are `captcha`, `mfa`, `agent`, and `heal_failed`. A run waits up to 30 minutes for `request.respond()` or `run.respond()`. Run records are held in server memory for one hour after completion; they do not survive a server restart.

### Data, secrets, and reusable placeholders

Use `{{name}}` references in prompts instead of interpolating values into the prompt text. `data` is available to the agent for reasoning; `secrets` supplies values for typing without including them as readable task inputs. Filters include `first`, `last`, `digits`, and `date:MM/DD/YYYY`.

For prompt runs, pass credentials in `secrets`. For replay, pass all variables in `data`: the saved playbook tracks which variables are secret, including during healing. Placeholder-based inputs remain variables in the saved flow. This is not a blanket redaction guarantee for page content, screenshots, agent replies, or application logs; inspect exported code before sharing it.

### Files

`file()` puts a file in `data`. The agent attaches it with its upload tool: hand it the element id of whatever you can see (the "Choose file" button, the drop zone, the field itself) and the real `<input type="file">` is found from there, including the hidden ones most upload widgets use.

```js
import { Oya, file } from '@oya-ai/browser';

await browser.ask('Attach my resume to the application and submit it', {
  data: { name: 'Ada Lovelace', resume: await file('./cv.pdf') },
});
```

A string argument is a path on disk (Node only); a `Blob`, a `File`, or a `Uint8Array` works anywhere. `name` sets the filename the site sees and `type` overrides the MIME guessed from the extension. The ceiling is 10MB per file, and the bytes travel inline with the run. Nothing is stored server-side after it ends.

Files work in `data` for `ask()`, `submit()`, and `play()`; `secrets` rejects them, because a file is never typed through a placeholder. A run recorded with `toPlaybook()` keeps the upload as a variable, so the replay takes a different file:

```js
await browser.play('job-application', { name: 'Ada Lovelace', resume: await file('./other.pdf') });
```

The generated Playwright module calls `setInputFiles`, where the same variable is a plain path rather than a `file()` value.

### Review a repaired playbook

Replay normally runs recorded steps without an LLM. With `autoHeal: true` (the default), a broken step can hand over to the agent, which saves a repair as `<name>:draft`. Promotion replaces the saved playbook with that draft.

The following continues with an active `browser` and the inputs above. Replaying a draft performs its actions, so review its code and use test inputs before promoting it.

```js
const saved = (await oya.playbooks.list()).find((p) => p.name === playbookName);
if (saved?.draft) {
  // Review saved.draft.code before executing it.
  await browser.play(`${playbookName}:draft`, { ...data, ...secrets }, { autoHeal: false });
  await oya.playbooks.promote(playbookName);
}
```

Use `autoHeal: false` with `play()` or `submit({ playbook: name }, options)` to fail at a broken step without agent repair. `oya.playbooks.remove(name)` removes a playbook and its draft; `remove("<name>:draft")` removes only the draft.

## Use your own model key

As in the portal-automation project's model setup script, configure the model on your Oya API key once for subsequent agent runs:

```js
import { Oya } from '@oya-ai/browser';

const modelKey = process.env.GEMINI_API_KEY;
if (!modelKey) throw new Error('Set GEMINI_API_KEY first.');
const oya = new Oya();
await oya.config.set({
  llm_provider: 'gemini', // LlmProvider: "openai" | "anthropic" | "gemini" | "vertex"
  openai_api_key: modelKey, // Shared field name for every supported provider.
  // chat_model: process.env.OYA_CHAT_MODEL, // Optional provider model override.
});
```

`config.set` takes `ConfigUpdate` and `config.get()` returns `Config`, so an editor offers the valid providers and a typo fails to compile rather than silently falling back:

```ts
import { Oya, type LlmProvider } from '@oya-ai/browser';

await oya.config.set({ llm_provider: 'vertx' });
//                                   ~~~~~~~ Type '"vertx"' is not assignable to type
//                                           'LlmProvider'. Did you mean '"vertex"'?

const provider: LlmProvider = 'vertex'; // for your own config plumbing
const { effective } = await oya.config.get();
console.log(effective.baseUrl, effective.model, effective.hasLlmKey);
```

`browser_provider` is typed as `Provider` and `captcha_solver` as `CaptchaSolver` the same way. The server enforces the same sets, so a non-TypeScript caller gets a 400 listing the valid values instead of a silent fallback. Pass `null` to clear a field.

### Gemini Enterprise (ex-Vertex AI)

`llm_provider: "vertex"` targets express mode, whose API keys work against a global endpoint with no GCP project or location:

```js
await oya.config.set({ llm_provider: 'vertex', openai_api_key: process.env.VERTEX_EXPRESS_KEY });
```

For an enterprise project instead, point at its OpenAI-compatible endpoint. That path authenticates with a Google OAuth access token rather than an API key, and the token expires after about an hour, so it suits a one-off run rather than a long-lived deployment:

```js
await oya.config.set({
  llm_provider: 'vertex',
  openai_base_url:
    'https://us-central1-aiplatform.googleapis.com/v1/projects/PROJECT/locations/us-central1/endpoints/openapi',
  openai_api_key: accessToken, // gcloud auth print-access-token
  chat_model: 'google/gemini-2.5-flash', // this endpoint prefixes model ids
});
```

Omit `chat_model` to use the configured provider's default. `oya.config.get()` reads configuration. To return to the shared model configuration:

```js
await oya.config.set({ llm_provider: null, openai_api_key: null, chat_model: null });
```

## Live view, sharing, and embedded streams

`browser.liveViewUrl()` returns a dashboard link without an API key; the viewer signs into Oya. For a scoped handoff to someone else, create an expiring share link:

```js
const share = await browser.shareUrl({ control: true, expiresInSeconds: 900 });
// Deliver share.url privately to the intended operator.
// Once the handoff is finished:
await browser.revokeShare(share.id);
```

Share links are view-only by default. `control: true` permits browser interaction. Anyone holding the link has its access until expiry or revocation.

For an embedded SSE stream of JPEG frames, use `await browser.liveStreamUrl()`. It mints a single-use connection ticket valid for 60 seconds; request a new URL for each connection.

---

## 🛡️ Deterministic Personas (Anti-Ban Identity)

A persona groups a stable device fingerprint, saved login cookies, and a proxy assignment for reuse across browser sessions.

```ts
import { Oya } from '@oya-ai/browser';

const oya = new Oya();

// Create a persistent persona
const persona = await oya.personas.create({
  name: 'us-shopper',
  prefs: { platform: 'MacIntel', timezone: 'America/New_York', locale: 'en-US' },
  proxy: { geo: 'US' },
  maxConcurrent: 2, // Limit simultaneous sessions for this identity
});

// Launch a browser with this persona (or persona: 'auto' for least-recently-used)
await using browser = await oya.browser.start({ persona: persona.id });
await browser.goto('https://www.amazon.com');

// Hardware attributes remain byte-identical on subsequent sessions
console.log(persona.fingerprint.platform, persona.fingerprint.timezone);

// Need another device of the same class? Clone it with an empty cookie jar:
// const altDevice = await oya.personas.clone(persona.id, { name: "us-shopper-02" });
```

---

## 🧠 Structured Agent Analysis & Interaction

Skip messy DOM traversal. Get clean markdown and numbered interactive elements:

```ts
import { Oya } from '@oya-ai/browser';

const oya = new Oya();
await using browser = await oya.browser.start();
await browser.goto('https://github.com/trending');

// Analyze page: returns markdown and visible numbered elements
const { markdown, elements } = await browser.analyze();
console.log(markdown.slice(0, 300));

// Interact using numbered element IDs:
const firstRepo = elements.find((el) => el.tag === 'a' && el.href?.includes('/stargazers'));
if (firstRepo) {
  await browser.click(firstRepo.id); // clicks [data-ac-id="firstRepo.id"]
}
```

---

## 🧩 Challenge Handling: Automated CAPTCHA & Sealed MFA

```ts
import { Oya } from '@oya-ai/browser';

const oya = new Oya();

// 1. Seal a TOTP secret on a persona (encrypted with AES-256-GCM at rest, never exposed over API)
const persona = await oya.personas.create({ name: 'finance-admin' });
await oya.personas.setMfa(persona.id, {
  type: 'totp',
  secret: process.env.TOTP_SECRET!,
});

await using browser = await oya.browser.start({ persona: persona.id });

// 2. Clear CAPTCHAs automatically (uses vendor solver or CapSolver/2Captcha fallback)
await browser.goto('https://www.google.com/recaptcha/api2/demo');
const captcha = await browser.solveCaptcha();
console.log('CAPTCHA Solved:', captcha.solved, 'via', captcha.method);

// 3. Complete Two-Factor Authentication
await browser.goto(process.env.MFA_LOGIN_URL!);
const mfa = await browser.completeMfa();

if (!mfa.completed && mfa.liveViewUrl) {
  // Hand off to human operator if interactive push notification or WebAuthn is needed
  console.log('Interactive handoff required at:', mfa.liveViewUrl);
}
```

---

## 🔌 Universal CDP Gateway (Playwright & Puppeteer)

Every browser exposes an authenticated `browser.cdpUrl` routed through Oya's gateway. Connect standard Playwright, Puppeteer, or Stagehand:

```ts
import { chromium } from 'playwright-core';
import { Oya } from '@oya-ai/browser';

const oya = new Oya();

// Run on any underlying provider: browserbase, steel, anchor, browseruse, or oya-cloud
await using browser = await oya.browser.start({ provider: 'browserbase' });

// Connect Playwright directly over Oya's gateway
const context = (await chromium.connectOverCDP(browser.cdpUrl!)).contexts()[0];
const page = context.pages()[0] ?? (await context.newPage());

await page.goto('https://news.ycombinator.com');
console.log('Page Title:', await page.title());
```

---

## API reference

### Initialization

```ts
import { Oya } from '@oya-ai/browser';

const oya = new Oya({
  apiKey: 'oya_...', // default: process.env.OYA_API_KEY
  baseUrl: 'https://oyabrowser.com', // default: OYA_BASE_URL, then the hosted service
  timeoutMs: 60_000, // per request
  // fetch: customFetch,                // any fetch-compatible implementation
});
```

### Browser Operations (`oya.browser`)

| Method               | Signature                                                                         | Description                                             |
| :------------------- | :-------------------------------------------------------------------------------- | :------------------------------------------------------ |
| `start(options)`     | `(options?: StartOptions) => Promise<Browser>`                                    | Start a browser and wait until it is ready for commands |
| `get(id)`            | `(id: string) => Promise<Browser>`                                                | Reattach to an existing running browser                 |
| `list()`             | `() => Promise<BrowserInfo[]>`                                                    | List all active running browser sessions                |
| `stop(ids \| 'all')` | `(ids: string[] \| 'all') => Promise<{ stopped: number; results: StopResult[] }>` | Stop target browsers or all browsers                    |
| `stopAll()`          | `() => Promise<number>`                                                           | Terminate all active browser sessions                   |

#### `StartOptions`

- `persona?: 'default' | 'auto' | string`, Assign persistent identity
- `provider?: 'oya-cloud' | 'oya-selfhosted' | 'browserbase' | 'steel' | 'anchor' | 'browseruse' | 'cdp'`
- `wsUrl?: string`: required only for the `'cdp'` provider. Either the WebSocket URL, or the
  plain `http://localhost:9222` that Chrome prints for `--remote-debugging-port`, which is
  resolved through Chrome's own `/json/version`
- `name?: string`: display name in the console and `oya ls`
- `captcha?: 'auto' | 'off'`, Automatically solve CAPTCHAs on navigation
- `queueMs?: number`, Wait duration for fleet capacity (ms)
- `budgetUsd?: number`, Enforce budget limit for session
- `idempotencyKey?: string`, Safe retry token
- `governed?: boolean`, Enable governed session controls
- `profile?: string`, Saved login profile (takes precedence over `persona`)
- `priority?: 'low' | 'normal' | 'high'`, Queue priority
- `policy?: { allowedHosts?, humanHosts?, region?, redactRecording? }`, Session policy
- `readyTimeoutMs?: number`, Wait budget for a starting browser to connect

### Browser Instance Methods (`browser.*`)

| Method                              | Returns                                      | Description                                              |
| :---------------------------------- | :------------------------------------------- | :------------------------------------------------------- |
| `goto(url)`                         | `Promise<void>`                              | Navigate to URL (with optional auto-CAPTCHA)             |
| `ask(prompt, { data?, secrets? }?)` | `Promise<string>`                            | Natural-language AI driving using key's configured model |
| `analyze()`                         | `Promise<Analysis>`                          | Returns markdown representation and numbered elements    |
| `elements()`                        | `Promise<Element[]>`                         | Returns only visible interactable elements               |
| `click(elementId)`                  | `Promise<void>`                              | Click element by numeric ID from `analyze()`             |
| `type(elementId, text)`             | `Promise<{ suggestions_visible?: boolean }>` | Type text into specified element                         |
| `pressKey(key)`                     | `Promise<void>`                              | Dispatch keyboard key event (e.g. `'Enter'`)             |
| `scroll(dir, amount?, at?)`         | `Promise<void>`                              | Scroll `'up' \| 'down' \| 'top' \| 'bottom'`             |
| `waitFor(selector, timeout?)`       | `Promise<void>`                              | Wait for DOM selector                                    |
| `screenshot()`                      | `Promise<string>`                            | Capture page as base64 image data URL                    |
| `url()`                             | `Promise<string>`                            | Current active tab URL                                   |
| `tabs()`                            | `Promise<Tab[]>`                             | List open tabs                                           |
| `openTab(url?)`                     | `Promise<string>`                            | Open a new tab                                           |
| `switchTab(tabId)`                  | `Promise<void>`                              | Switch active tab                                        |
| `closeTab(tabId)`                   | `Promise<void>`                              | Close target tab                                         |
| `solveCaptcha()`                    | `Promise<CaptchaResult>`                     | Detect and solve on-screen CAPTCHA                       |
| `completeMfa()`                     | `Promise<MfaResult>`                         | Resolve TOTP/SMS MFA or return `liveViewUrl`             |
| `liveViewUrl()`                     | `string`                                     | Dashboard link for this browser                          |
| `liveStreamUrl()`                   | `Promise<string>`                            | SSE frame stream URL with a single-use ticket            |
| `shareUrl(options?)`                | `Promise<{ url, id, expiresAt }>`            | Expiring browser share link; optional control access     |
| `revokeShare(id)`                   | `Promise<void>`                              | Revoke a share link                                      |
| `submit(task, options?)`            | `Promise<Run>`                               | Background prompt or playbook with callbacks             |
| `toPlaybook(name)`                  | `Promise<Playbook>`                          | Save the latest agent flow and export Playwright code    |
| `play(name, data?, { autoHeal? }?)` | `Promise<PlayResult>`                        | Replay a saved flow                                      |
| `status()`                          | `Promise<BrowserDetail>`                     | Instance metrics, health, and recent activity log        |
| `stop()`                            | `Promise<StopResult>`                        | Tear down sandbox and release CDP session                |

### Profile and persona management (`oya.profiles`, `oya.personas`)

`oya.profiles` exposes the same methods as `oya.personas`; the persona name remains available for existing integrations.

| Method                                              | Description                                                       |
| :-------------------------------------------------- | :---------------------------------------------------------------- |
| `create({ name?, prefs?, proxy?, maxConcurrent? })` | Create new deterministic device identity                          |
| `list()`                                            | List all saved personas and active concurrency                    |
| `get(id)`                                           | Get persona profile details                                       |
| `update(id, changes)`                               | Update name, concurrency limit, or proxy geo                      |
| `clone(id, options)`                                | Create fresh persona with same device traits but empty cookie jar |
| `preview(prefs)`                                    | Preview generated hardware fingerprint before creating            |
| `options()`                                         | Available platforms, timezones, and valid locales                 |
| `pinProxy(id, proxyId)`                             | Bind persona permanently to a residential proxy exit node         |
| `remove(id)`                                        | Delete persona and associated cookie jar                          |
| `setMfa(id, config)`                                | Store TOTP secret (sealed at rest with AES-256-GCM)               |
| `clearMfa(id)`                                      | Remove MFA secret from persona                                    |
| `cookies(id, format?)` | Export the persona's logins; `'playwright'` fits `addCookies()` |
| `importCookies(id, cookies)` | Merge cookies into the jar (from a file, a script, anywhere) |
| `copyCookies(from, to)` | Copy one persona's logins into another |
| `clearCookies(id)` | Forget every cookie: signs the persona out everywhere |

### Proxies (`oya.proxies`)

| Method                                               | Description                                                                       |
| :--------------------------------------------------- | :-------------------------------------------------------------------------------- |
| `create({ url, label?, geo?, kind?, maxPersonas? })` | Add a proxy from your vendor. Credentials are encrypted and never returned        |
| `list()`                                             | Your proxies and shared ones, with exit IP, health and how many personas use each |
| `check()`                                            | Dial every proxy and record its real exit IP                                      |
| `remove(id)`                                         | Delete a proxy and unpin the personas on it                                       |

```ts
const proxy = await oya.proxies.create({
  url: 'http://user:pass_session-shopper1@gate.vendor.com:7000', // one sticky session per persona
  label: 'us-shopper-1',
  geo: 'US',
  kind: 'residential',
  maxPersonas: 1,
});
await oya.personas.pinProxy(persona.id, proxy.id);
```

### Durable Governance & Control (`oya.control`)

| Method                                             | Description                                                              |
| :------------------------------------------------- | :----------------------------------------------------------------------- |
| `overview()`                                       | Fleet overview, spend, sessions, and active rate cards                   |
| `sessions()`                                       | List all durable sessions (including cleanup-pending)                    |
| `session(id)`                                      | Get detailed session execution state                                     |
| `takeover(id, 'acquire' \| 'release' \| 'resume')` | Manage human control leases                                              |
| `ticket(id)`                                       | Generate single-use connection ticket for secure handoff                 |
| `events(after?)`                                   | Read audit events and a pagination cursor                                |
| `createCredential(options)`                        | Mint scoped service credential (`viewer` / `operator` / `administrator`) |
| `createWebhook(url, types)`                        | Register HMAC-signed webhook for fleet lifecycle events                  |

---

## 🚨 Error Handling

API error responses and failed browser commands throw `OyaError`. Network failures, request timeouts, and configuration errors may throw other error types:

```ts
import { Oya, OyaError } from '@oya-ai/browser';

try {
  const oya = new Oya();
  await oya.browser.start({ persona: 'invalid-id' });
} catch (err) {
  if (err instanceof OyaError) {
    console.error(`Oya API Error (${err.status}):`, err.message);
    // err.body contains response details; inspect privately if needed.
  } else {
    throw err;
  }
}
```

---

## 📄 License

MIT © [Oya](https://getoya.ai)
