<p align="center">
  <img src="assets/oya-banner.svg" alt="Oya Browser — the browser control plane for AI agents" width="100%">
</p>

<h3 align="center">OpenRouter for browsers.</h3>

<p align="center">
  One API over Oya Cloud, Browserbase, Steel, Anchor, Browser Use and your own Chrome.<br>
  Turn a portal task into a reusable playbook. Replay with new inputs, review agent repairs, and bring in a person when a run needs help.
</p>

<p align="center">
  <a href="#stealth-0-headless-0-lies"><img src="https://img.shields.io/badge/CreepJS_headless-0%25-39ed35" alt="CreepJS headless score: 0%"></a>
  <a href="#stealth-0-headless-0-lies"><img src="https://img.shields.io/badge/CreepJS_lies-0-39ed35" alt="CreepJS lies detected: 0"></a>
  <a href="#stealth-0-headless-0-lies"><img src="https://img.shields.io/badge/Sannysoft-31%2F31-39ed35" alt="Bot.Sannysoft: 31 of 31 checks pass"></a>
  <br>
  <a href="https://www.npmjs.com/package/@oya-ai/browser"><img src="https://img.shields.io/npm/v/@oya-ai/browser?color=39ed35&label=@oya-ai/browser&logo=npm" alt="npm: @oya-ai/browser"></a>
  <a href="https://www.npmjs.com/package/@oya-ai/cli"><img src="https://img.shields.io/npm/v/@oya-ai/cli?color=39ed35&label=@oya-ai/cli&logo=npm" alt="npm: @oya-ai/cli"></a>
  <a href="LICENSE.md"><img src="https://img.shields.io/badge/license-Sustainable_Use_%2B_MIT_SDK-157a13" alt="License"></a>
  <a href="https://browser.getoya.ai"><img src="https://img.shields.io/badge/hosted-browser.getoya.ai-141410?logo=googlechrome&logoColor=39ed35" alt="Hosted"></a>
</p>

<p align="center">
  <a href="https://browser.getoya.ai">Hosted</a> ·
  <a href="#self-host">Self-host</a> ·
  <a href="https://browser.getoya.ai/docs">Docs</a> ·
  <a href="examples">Examples</a>
</p>

<p align="center">
  <img src="assets/oya-fleet-demo.gif" alt="Oya fleet console walkthrough" width="100%">
</p>
<p align="center"><sub>Three real browsers on a self-hosted stack: live view, a human takeover, numbered elements, and the hand-back. Nothing mocked (<a href="scripts/record-walkthrough.mjs"><code>scripts/record-walkthrough.mjs</code></a>).</sub></p>

## ⚡ Quick start

```bash
npm install @oya-ai/browser
```

Set `OYA_API_KEY` from [browser.getoya.ai](https://browser.getoya.ai). The SDK supports Node.js 18+; this example uses ES modules.

```js
import { Oya } from "@oya-ai/browser";

const oya = new Oya(); // Reads OYA_API_KEY; optional OYA_BASE_URL for self-hosting.
const browser = await oya.browser.start({ captcha: "auto" });
try {
  await browser.goto("https://example.com");
  console.log(await browser.ask("What is the main heading?"));
} finally {
  await browser.stop();
}
```

Choose the provider in your key settings or pass `provider` to `start()`. On Node.js 24+, `await using browser = await oya.browser.start()` handles cleanup automatically.

## Portal automation: record once, replay with new inputs

Adapted from the portal-automation project, with a fictional request and environment-based credentials. Set `PORTAL_URL`, `PORTAL_USERNAME`, and `PORTAL_PASSWORD` to your test portal and adapt the instructions to its pages. Save this as `portal.mjs` and run `node portal.mjs`.

```js
import { createInterface } from "node:readline/promises";
import { Oya } from "@oya-ai/browser";

function requiredEnv(name) {
  const value = process.env[name];
  if (!value) throw new Error(`Set ${name} before running.`);
  return value;
}

const oya = new Oya({ apiKey: requiredEnv("OYA_API_KEY") });
const portalUrl = requiredEnv("PORTAL_URL");
const name = "portal-request-review";
const data = { customerName: "Alex Example", requestId: "DEMO-0001" };
const secrets = {
  username: requiredEnv("PORTAL_USERNAME"),
  password: requiredEnv("PORTAL_PASSWORD"),
};
const persona = (await oya.personas.list()).find((p) => p.name === "portal-demo")
  ?? await oya.personas.create({ name: "portal-demo" });
const browser = await oya.browser.start({ persona: persona.id, captcha: "auto" });

try {
  await browser.goto(portalUrl);
  const exists = (await oya.playbooks.list()).some((p) => p.name === name);
  const run = await browser.submit(
    exists ? { playbook: name } : { prompt: [
      "If needed, log in with {{username}} and {{password}}.",
      "Open New Request. Enter {{customerName}} and reference {{requestId}}.",
      "Stop on the review page. Do not submit the request.",
    ].join("\n") },
    {
      ...(exists ? { data: { ...data, ...secrets } } : { data, secrets }),
      onHealed: (result) => console.log("Repair draft ready:", result.draft),
      onHumanAttention: async (request) => {
        console.log("Attention needed:", request.reason);
        console.log("Open:", request.liveViewUrl ?? browser.liveViewUrl());
        console.log(request.message);
        const terminal = createInterface({ input: process.stdin, output: process.stdout });
        try {
          const answer = await terminal.question(request.reason === "agent"
            ? "Answer the agent: "
            : "Handle this in the live view, then press Enter: ");
          await request.respond(answer || "done");
        } finally {
          terminal.close();
        }
      },
    },
  );
  await run.done; // Rejects on failure; save only a successful run.
  if (!exists) await browser.toPlaybook(name);
} finally {
  await browser.stop();
}
```

- **Reusable inputs:** `data` is readable by the agent; `secrets` provides credentials for typing. Refer to both through `{{placeholders}}`. Replay takes all variables in `data` and remembers which names are secret.
- **Repair drafts:** a broken replay can hand over to the agent and save `<name>:draft`. Review and test it before `oya.playbooks.promote(name)`. Set `autoHeal: false` to fail without agent repair.
- **Human handoff:** `submit()` reports CAPTCHA, MFA, agent questions, and failed healing through `onHumanAttention`. The run waits up to 30 minutes for a response. `run.done` settles when it ends; `run.status()` reads its record.
- **Browser reuse:** `oya.browser.get(id)` attaches to an existing browser. Stop only the browser your application owns.

The [full SDK example](packages/sdk/README.md#portal-automation-record-once-replay-with-new-inputs) includes a working terminal response handler, browser reuse, and draft review. Keep input values out of prompt text and inspect exported playbooks before sharing; placeholders do not redact every page, screenshot, or log.

## Your model, your live view

Configure your own model key once for subsequent agent runs:

```js
const modelKey = process.env.GEMINI_API_KEY;
if (!modelKey) throw new Error("Set GEMINI_API_KEY first.");
await oya.config.set({
  llm_provider: "gemini", // Also supports "openai", "anthropic" and "vertex" (Gemini Enterprise).
  openai_api_key: modelKey, // Shared field name for every provider.
});
```

`browser.liveViewUrl()` opens the signed-in dashboard. `await browser.shareUrl({ control: true, expiresInSeconds: 900 })` creates an expiring operator link; omit `control` for view-only access. Deliver it privately and revoke it with `browser.revokeShare(share.id)`. For embedded JPEG frames over SSE, `await browser.liveStreamUrl()` mints a single-use connection ticket.

See the [SDK README](packages/sdk/README.md) for the current API reference and [`examples/`](examples) for runnable CAPTCHA, MFA, persona, provider, and Playwright examples. The shorter snippets below assume an initialized `oya` client and, where needed, an active `browser`.

## 🧠 Give it to your agent

Get a key at [browser.getoya.ai](https://browser.getoya.ai), `export OYA_API_KEY=...`, then pick one:

```bash
# Claude Code — MCP server and skill, as one plugin
claude plugin marketplace add OyadotAI/oya-browser && claude plugin install oya-browser@oya

# Any agent that reads skills — Claude Code, Cursor, Codex, Copilot
npx skills add OyadotAI/oya-browser

# Just the MCP server
claude mcp add --transport http oya https://browser.getoya.ai/mcp/pool \
  --header "Authorization: Bearer $OYA_API_KEY"
```

Cursor, Windsurf, Claude Desktop or any other MCP client:

```json
{ "mcpServers": { "oya": { "url": "https://browser.getoya.ai/mcp/pool", "headers": { "Authorization": "Bearer YOUR_API_KEY" } } } }
```

Then ask it: *"Start a browser, open Hacker News and summarize the top 3 stories."* It calls `start_browser`, `navigate`, `analyze_page` and `stop_browser` on its own — **16 MCP tools**, all described at [llms.txt](https://browser.getoya.ai/llms.txt).

<a id="stealth-0-headless-0-lies"></a>

## 🕵️ Stealth: 0% headless, 0 lies

> **"Zero detection" is neither measurable nor achievable** — the published leader sits near 77% bypass. This produces a number instead.
>
> — [`server/test-stealth.js`](server/test-stealth.js)

The same headless Chrome, launched twice: once bare, once with a persona applied exactly as production does. Both then face the public detectors. Chrome 153 on macOS, 2026-09-11:

| | Bare headless Chrome | With an Oya persona |
|:---|:---:|:---:|
| **CreepJS headless score** (lower is better) | 100% | **0%** |
| **CreepJS lies detected** | 0 | **0** |
| **CreepJS stealth-tampering score** (lower is better) | 0% | **0%** |
| **Bot.Sannysoft** | 27 / 31 pass | **31 / 31 pass** |
| **Oya probe suite** (29 probes, weighted, 64 points) | 55 / 64 | **64 / 64** |
| CreepJS like-headless score (lower is better) | 38% | 31% |

Zero lies means CreepJS caught none — including the checks it runs from a second realm and from a service worker, where most stealth layers fail. Why it holds:

- 🧩 **Native first.** Chrome emulates the webdriver flag, platform, core count, locale, timezone and screen itself over CDP. There is no patched value to catch.
- 🎭 **Native-shaped patches.** What emulation can't reach is patched to look native from every realm: not constructible, no `prototype`, `[native code]` in every frame, "Illegal invocation" off the prototype — including CreepJS's phantom iframe.
- 🧵 **Workers and iframes too.** Dedicated, shared and service workers plus cross-site iframes, where CAPTCHA widgets live. Each is held paused until covered, so page and workers report one machine.

Of the remaining like-headless signals, two are headless-only rendering defaults; the other three are Android-only APIs real desktop Chrome lacks as well. Don't take our word for it:

```bash
oya stealth-test --live    # from a checkout, or: node server/test-stealth.js --live
```

## 🧬 Personas

Anti-bot systems watch device consistency over time, and there are two ways to fail it:

1. **One account, many fingerprints** looks like a bot farm.
2. **One fingerprint, many concurrent sessions** looks like a device farm.

```
persona = fingerprint + cookie jar + proxy    # one device
API key = a fleet of personas
```

A fingerprint is derived from a stored seed, so it is identical on every restart, and a persona is never re-rolled. Need another device of the same kind? Clone it.

```ts
const persona = await oya.personas.create({
  name: "us-shopper",
  prefs: { platform: "MacIntel", timezone: "America/New_York", locale: "en-US" },
  proxy: { geo: "US" },
  maxConcurrent: 2,                           // one device shouldn't run 50 sessions
});

await using browser = await oya.browser.start({ persona: persona.id });
console.log(persona.fingerprint.platform, persona.fingerprint.timezone);  // same next week
```

## 🙋 When automation hits a wall

Scripted logins break on Google SSO, Okta, passkeys and Cloudflare. Skip them: sign in **once** in the [desktop app](https://browser.getoya.ai), and every remote browser for that persona starts already signed in, on the same fingerprint. Cookies are sealed with AES-256-GCM.

```ts
await browser.goto("https://www.google.com/recaptcha/api2/demo");

const captcha = await browser.solveCaptcha();  // vendor's solver, else CapSolver / 2Captcha
const mfa = await browser.completeMfa();       // TOTP from a seed sealed on the persona

// A phone approval or biometric prompt: hand it to a person
if (!mfa.completed && mfa.liveViewUrl) console.log("Needs a human:", mfa.liveViewUrl);
```

| | How Oya handles it |
|:---|:---|
| 🧩 **CAPTCHA** | The vendor's native solver on Anchor, Browserbase and Steel — so you aren't billed twice and two solvers never race. 2Captcha or CapSolver everywhere else. |
| 🔑 **MFA** | TOTP seeds encrypted at rest with AES-256-GCM, never returned by the API. |
| 🙋 **Takeover** | A JPEG stream over SSE that accepts clicks, drags, scrolls and typing. An operator takes control, clears the prompt, releases, and the agent resumes. |

<p align="center">
  <img src="assets/oya-console-panel.png" alt="Live view with human takeover: an operator drives the browser and hands control back" width="100%">
</p>

## 🔀 Why Oya

Every cloud-browser vendor has its own API, session model and outages — couple your agents to one and you inherit all three.

<p align="center">
  <img src="assets/architecture.svg" alt="Agents connect over CDP, MCP, REST or CLI to the Oya control plane, which routes to Oya Cloud, Browserbase, Steel, Anchor, Browser Use or private Chrome" width="100%">
</p>

| | One vendor, directly | Through Oya |
|:---|:---|:---|
| **Switching vendors** | Rewrite against a new API | Change a setting on the key |
| **Vendor outage at connect** | Your agents are down | `/connect` falls through to the next route |
| **Device identity** | Whatever the vendor offers per session | A persona: seeded fingerprint, cookie jar and proxy, stable across runs |
| **Stealth** | The vendor's claims | [0% CreepJS headless, 0 lies, 31/31 Sannysoft](#stealth-0-headless-0-lies), reproducible with `oya stealth-test --live` |
| **Logins** | Scripted login flows | Sign in once on the desktop; remote personas inherit the cookies |
| **CAPTCHA and MFA** | Vendor-specific, or build it yourself | Native solver where there is one, CapSolver or 2Captcha otherwise, sealed TOTP, live takeover |
| **Repeatable tasks** | Build and maintain your own scripts | Record playbooks, replay with new inputs, and review agent repair drafts |
| **Fleet operations** | One dashboard per vendor | One console, Prometheus `/metrics`, an audit log, spend per key, stop-all |

**6 backends** — Oya Cloud, Browserbase, Steel, Anchor, Browser Use, your own Chrome — behind **4 surfaces**: CDP, MCP, REST and the SDK.

## 🛠️ Bring your own tools

Every browser exposes a `cdpUrl` routed through Oya's gateway, so Playwright, Puppeteer, Stagehand and browser-use all work unchanged:

```ts
import { chromium } from "playwright-core";

await using browser = await oya.browser.start({ provider: "browserbase" });

const context = (await chromium.connectOverCDP(browser.cdpUrl!)).contexts()[0];
const page = context.pages()[0] ?? (await context.newPage());
await page.goto("https://github.com/trending");
```

## 💻 CLI

<p align="center">
  <img src="assets/cli-demo.svg" alt="oya start, goto, ask and ls in a terminal" width="100%">
</p>

```bash
npm install -g @oya-ai/cli

oya login                        # save an API key
oya start --persona auto         # start a browser
oya goto https://example.com     # navigate the newest one
oya ask "Find the pricing tier"  # drive it in plain language
oya open                         # watch it live
oya ls                           # what's running
oya rm --all                     # stop everything
```

Full command list in the [CLI README](packages/cli).

## 📺 Console

<p align="center">
  <img src="assets/oya-fleet-browsers.png" alt="Oya fleet console: every connected browser, its persona, health and current URL" width="100%">
</p>

- 🩺 **Health strip** — healthy, stale, error and unresponsive counts. Click one to filter.
- 👀 **Live view** — watch any browser, or take control and drive it.
- 🔢 **Element tree** — numbered element IDs (`[data-ac-id]`), so an agent plans with fewer tokens.
- 🛡️ **Governance** — audit log, Prometheus `/metrics`, spend per key, and stop-all.

<a id="self-host"></a>

## 🚀 Self-host

```bash
git clone https://github.com/OyadotAI/oya-browser.git
cd oya-browser
make wizard
```

Six questions — where it runs, which database, where browsers run, which LLM, the public URL, optional services. It writes the config, builds the images, brings the stack up and waits for `/readyz` before telling you it worked.

```
◆ Database 2/6
  ❯ SQLite          zero config, one replica
    Supabase        adds email sign-in
    Postgres        many replicas
```

SQLite, Postgres or Supabase. Browsers on Docker, Kubernetes, Oya Cloud, a hosted vendor, or your own Chrome. **Full guide → [docs/self-hosting.md](docs/self-hosting.md)** · operations → [docs/control-plane.md](docs/control-plane.md)

## 📦 Packages

| Package | Description |
|:---|:---|
| [`@oya-ai/browser`](packages/sdk) | TypeScript SDK. ESM and CJS, typed, **zero runtime dependencies**. |
| [`@oya-ai/cli`](packages/cli) | CLI for the fleet, the live view and stealth tests. |
| [`server`](server) | Control plane: gateway, admission, personas, challenges. |
| [`ui`](ui) | Next.js console, live viewer and docs. |
| [`browser`](browser) | Containerized and Electron desktop runtime. |
| [`examples`](examples) | One runnable script per capability. |

## 🧪 Tests

```bash
npm test                 # gateway, providers, personas, security, challenges,
                         # the install wizard, the prompt layer and the k8s fleet
oya stealth-test --live  # the stealth numbers above, on your machine
```

## 📄 License

The SDK ([`packages/sdk`](packages/sdk)) and CLI ([`packages/cli`](packages/cli)) are MIT, so you can embed them in commercial agents. Everything else is source-available under the [Sustainable Use License](LICENSE.md): free for internal business use, research and non-commercial use.
