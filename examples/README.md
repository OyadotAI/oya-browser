# Oya Browser examples

<p align="center">
  <strong>Short TypeScript scripts that run. Each one is the smallest thing that shows a feature working.</strong>
</p>

---

## 📂 Example Catalog

Every script needs `OYA_API_KEY`, and runs from this `examples/` folder. The multi-vendor, CAPTCHA and MFA scripts also need the settings listed under [Lighting up every step](#️-lighting-up-every-step-in-settings).

### 🧰 [`sdk/`](sdk/): the Oya SDK on its own

| File | Feature Demonstrated | Run Command |
|:---|:---|:---|
| [`01-quickstart.ts`](sdk/01-quickstart.ts) | Start a browser, or use the one already connected, and drive it in plain language | `npx tsx --env-file=.env sdk/01-quickstart.ts` |
| [`02-captcha.ts`](sdk/02-captcha.ts) | Automated reCAPTCHA detection and resolution | `npx tsx --env-file=.env sdk/02-captcha.ts` |
| [`03-mfa.ts`](sdk/03-mfa.ts) | Password plus TOTP sign-in: the login and seed are sealed on the persona, the agent never sees them | `npx tsx --env-file=.env sdk/03-mfa.ts` |
| [`04-personas.ts`](sdk/04-personas.ts) | Stable device identities: create, reuse, rotate, and clone fingerprints | `npx tsx --env-file=.env sdk/04-personas.ts` |
| [`05-playbooks.ts`](sdk/05-playbooks.ts) | Ask once with hidden data, save it as a playbook, replay it without the LLM, and get callbacks with auto-heal | `npx tsx --env-file=.env sdk/05-playbooks.ts` |
| [`06-sign-in.ts`](sdk/06-sign-in.ts) | Sign in with a stored username and password | `npx tsx --env-file=.env sdk/06-sign-in.ts` |
| [`07-prompt-login.ts`](sdk/07-prompt-login.ts) | Sign in from a prompt: login and TOTP seed passed as secrets, typed as `{{password}}` and `{{seed\|totp}}` | `npx tsx --env-file=.env sdk/07-prompt-login.ts` |
| [`demo.ts`](sdk/demo.ts) | 🎬 Interactive tour of every capability | `npx tsx --env-file=.env sdk/demo.ts` |

### 🎭 [`playwright/`](playwright/): bring your own tools

| File | Feature Demonstrated | Run Command |
|:---|:---|:---|
| [`01-connect.ts`](playwright/01-connect.ts) | Connect standard Playwright directly over Oya's universal CDP gateway | `npx tsx --env-file=.env playwright/01-connect.ts` |
| [`reddit/`](playwright/reddit/) | 🔎 Scrape Reddit with 10 parallel browsers through a residential-proxy profile | `npx tsx --env-file=.env playwright/reddit/scrape.ts` |

### 🌐 [`multi-vendor/`](multi-vendor/): one API, many clouds

| File | Feature Demonstrated | Run Command |
|:---|:---|:---|
| [`01-multi-vendor.ts`](multi-vendor/01-multi-vendor.ts) | Execute identical automation across **Oya Cloud, Browserbase, Steel, Anchor, and Browser Use** in parallel | `npx tsx --env-file=.env multi-vendor/01-multi-vendor.ts` |

### ☁️ [`ecs/`](ecs/): Oya browsers in your own AWS account

Each one sets `sandbox_runtime: 'ecs'` and a nested `ecs` setting, then starts a browser as a Fargate task in your account. They need the `ECS_*` and `AWS_*` values in [`.env.example`](.env.example), and a task definition whose `browser` container runs the Oya browser image.

| File | Feature Demonstrated | Run Command |
|:---|:---|:---|
| [`01-iam.ts`](ecs/01-iam.ts) | Sign in with an IAM access key | `npx tsx --env-file=.env ecs/01-iam.ts` |
| [`02-role.ts`](ecs/02-role.ts) | Oya assumes a role you trust with your own ExternalId: no long-lived keys | `npx tsx --env-file=.env ecs/02-role.ts` |
| [`03-sso.ts`](ecs/03-sso.ts) | Sign in with an IAM Identity Center (SSO) access token | `npx tsx --env-file=.env ecs/03-sso.ts` |
| [`04-cluster-name.ts`](ecs/04-cluster-name.ts) | Name the cluster by its name, looked up in `region` | `npx tsx --env-file=.env ecs/04-cluster-name.ts` |
| [`05-cluster-arn.ts`](ecs/05-cluster-arn.ts) | Name the cluster by its full ARN, whose region must match `region` | `npx tsx --env-file=.env ecs/05-cluster-arn.ts` |

---

## ⚡ Setup in 60 Seconds

### Prerequisites
- Node.js **20.6+** (native `--env-file` support)
- An Oya API key from [oyabrowser.com](https://oyabrowser.com) or your local self-hosted instance (`http://localhost:3100`)

### Installation

```bash
git clone https://github.com/OyadotAI/oya-browser.git
cd oya-browser/examples

# Install dependencies (automatically builds the SDK from ../packages/sdk)
npm install

# Configure your API key
cp .env.example .env
# Edit .env and paste your API key after OYA_API_KEY=
```

> **Tip:** If `OYA_API_KEY` is exported in your active terminal session, it takes precedence over `.env`. Run `unset OYA_API_KEY` first if you want to use `.env`.

---

## 🎬 The Grand Interactive Tour: `sdk/demo.ts`

[`sdk/demo.ts`](sdk/demo.ts) is an end-to-end interactive showcase that guides you through every capability, pausing for `Enter` between each step:

```bash
# Interactive mode (pauses between steps)
npx tsx --env-file=.env sdk/demo.ts

# Non-stop mode (runs straight through)
npx tsx --env-file=.env sdk/demo.ts --no-pause
```

### What the Tour Executes:

1. **Multi-Vendor Orchestration:** Tests starting browsers across configured providers.
2. **Deterministic Persona Creation:** Seeds a stable hardware profile and residential exit IP.
3. **Target Navigation:** Reads a real Amazon product page.
4. **CAPTCHA Solving:** Detects challenge puzzles and dispatches solvers.
5. **High-Res Screenshot Capture:** Captures and reports page viewport states.
6. **Natural Language Driving:** Prompts an LLM to navigate and parse data.
7. **Parallel Fleet Coordination:** Runs multi-browser operations simultaneously.
8. **Automated Two-Factor Login:** Solves real TOTP prompts at `authenticationtest.com`.

---

## ⚙️ Lighting Up Every Step in Settings

If a step reports a capability is unconfigured, you can activate it in the Control Plane Dashboard (**Settings**):

1. **AI Model:** Select Claude (`claude-sonnet-4-5`) or OpenAI (`gpt-4o-mini`) and provide an API key. Enables natural language driving.
2. **Browser Providers:** Configure keys for Browserbase, Steel, Anchor, or Browser Use. Enables multi-vendor fleet spreading.
3. **CAPTCHA Solvers:** Configure CapSolver or 2Captcha keys for providers without native challenge solving.
4. **Two-Factor Login:** Uncomment the test credentials in `.env` (provided in `.env.example`) to run the live MFA verification step.

---

## 📄 License

[Sustainable Use License](../LICENSE.md). Only `packages/sdk` and `packages/cli` are MIT.
