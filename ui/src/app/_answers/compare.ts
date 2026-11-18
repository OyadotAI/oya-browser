/**
 * What Oya and the vendors buyers compare it with say about themselves, cell by cell, for the
 * comparison tables on the answer pages. Each competitor cell paraphrases that vendor's own public
 * page, listed in its sources and read on CHECKED; "Not stated" means the pages read did not say,
 * never that the vendor lacks it. One module, so the overview tables and the per-vendor pages cannot
 * disagree, and a refresh changes every page at once.
 */
import type { AnswerSource, AnswerTable } from './content';

/** When the vendor pages below were read. Re-read them and move this date together. */
export const CHECKED = '2026-11-18';

/** Shown where a vendor's pages did not say. */
const NS = 'Not stated';

/** A vendor in a comparison: its column header, its cells by row key, and where they come from. */
export interface Vendor {
  /** Its column header. */
  name: string;
  /** Its cell for each row, keyed as the rows are. */
  cells: Record<string, string>;
  /** Its own pages the cells come from. */
  sources: AnswerSource[];
}

/** One row of a comparison: its key, and the label in its first column. */
interface Row {
  /** How vendors key their cell for it. */
  key: string;
  /** What the row compares, in a buyer's words. */
  label: string;
}

/** Builds a table from rows and vendors; a vendor that says nothing about a row gets "Not stated". */
export function compareTable(caption: string, rows: Row[], vendors: Vendor[]): AnswerTable {
  return {
    caption,
    columns: ['', ...vendors.map((v) => v.name)],
    rows: rows.map(({ key, label }) => [label, ...vendors.map((v) => v.cells[key] ?? NS)]),
  };
}

/** Every vendor's sources, once each, for a page's sources list. */
export const sourcesOf = (vendors: Vendor[]): AnswerSource[] => vendors.flatMap((v) => v.sources);

/** What a team running browsers for agents checks, in order of what rules a vendor out first. */
export const INFRA_ROWS: Row[] = [
  { key: 'baa', label: 'Signs a BAA for HIPAA work' },
  { key: 'soc2', label: 'SOC 2' },
  { key: 'selfHost', label: 'Runs in your own infrastructure' },
  { key: 'replay', label: 'Replays your finished task with no model' },
  { key: 'code', label: 'Replay is standard Playwright you own' },
  { key: 'audit', label: 'Audit trail of agent and person actions' },
  { key: 'secrets', label: 'Model never sees passwords' },
  { key: 'takeover', label: 'A person can take over live' },
  { key: 'providers', label: 'Runs browsers on other vendors behind one API' },
  { key: 'price', label: 'Free tier, then first paid plan' },
];

/** Oya, in the site's own words (see _home/content.ts and OYA_CONTROLS). */
export const OYA: Vendor = {
  name: 'Oya',
  cells: {
    baa: 'Yes, on Oya Cloud and the enterprise edition',
    soc2: 'Type II audit in progress',
    selfHost:
      'Enterprise edition on Docker, ECS, GCP or Kubernetes, with a BAA; community edition free up to 5 browsers',
    replay: 'Yes: playbooks, and the agent repairs only a step that moved',
    code: 'Yes: every playbook is a readable Playwright module that runs on any Chromium browser over CDP',
    audit: 'Hash-chained rows the database refuses to edit or delete, kept 365 days at least',
    secrets: 'Passwords and TOTP seeds are sealed and never shown to the model',
    takeover: 'Watch live, take over, hand back',
    providers: 'Yes: Oya Cloud, Browserbase, Steel, Anchor, Browser Use or your own Chrome, with failover',
    price: 'Free desktop app and 1 cloud browser hour; $20 a month for 100 hours',
    kind: 'Self-serve: desktop app, Oya Cloud, or your own infrastructure',
    mfa: 'Sealed TOTP; staff take over for a text-message code and hand back',
  },
  sources: [],
};

/** Browserbase, from its pricing, security, Stagehand and live view pages. */
export const BROWSERBASE: Vendor = {
  name: 'Browserbase',
  cells: {
    baa: 'On the Scale plan',
    soc2: 'Type II',
    selfHost: 'No on-prem; VPC connectivity and private cloud for large enterprise customers',
    replay: "Through Stagehand's cache, inside its own SDK",
    code: "Not stated: the cache is Stagehand's, not exported as Playwright",
    secrets: '1Password fills logins without returning the values to your application',
    takeover: 'Yes, live view',
    price: '1 browser hour free; $20 a month for 100 hours',
  },
  sources: [
    { label: 'Browserbase pricing', url: 'https://www.browserbase.com/pricing' },
    { label: 'Browserbase security docs', url: 'https://docs.browserbase.com/account/enterprise/security' },
    { label: 'Stagehand caching docs', url: 'https://docs.stagehand.dev/v3/best-practices/caching' },
    { label: 'Browserbase live view docs', url: 'https://docs.browserbase.com/features/session-live-view' },
  ],
};

/** Steel, from its pricing, security, GitHub and credentials pages. */
export const STEEL: Vendor = {
  name: 'Steel',
  cells: {
    baa: 'On the Scale and Enterprise plans',
    selfHost: 'Steel Browser self-hosts with Docker (Apache-2.0)',
    replay: 'No built-in replay; you turn a trace into a script yourself',
    secrets: "Credentials never reach your agent's model (credentials in beta)",
    takeover: 'Yes, live view accepts mouse and keyboard',
    price: '$0 plus usage at $0.10 an hour; $250 a month',
  },
  sources: [
    { label: 'Steel pricing', url: 'https://steel.dev/pricing' },
    { label: 'Steel security and compliance', url: 'https://steel.dev/docs/platform/security-and-compliance' },
    { label: 'Steel Browser on GitHub', url: 'https://github.com/steel-dev/steel-browser' },
    { label: 'Steel credentials docs', url: 'https://docs.steel.dev/overview/credentials-api/overview' },
  ],
};

/** Anchor, from its pricing, security, Action Index and secrets pages. */
export const ANCHOR: Vendor = {
  name: 'Anchor',
  cells: {
    baa: 'On the Enterprise plan',
    soc2: 'Type II',
    selfHost: 'Bring your own cloud or on-prem, on the Enterprise plan',
    replay: "Its Action Index replays Anchor's catalog of actions, not your own finished task",
    audit: 'Tamper-evident log of administrative actions',
    secrets: 'Secret values are never exposed to the AI model',
    takeover: 'Yes, one-click live takeover',
    price: '5 free credits; $50 a month',
  },
  sources: [
    { label: 'Anchor pricing', url: 'https://anchorbrowser.io/pricing' },
    { label: 'Anchor security docs', url: 'https://docs.anchorbrowser.io/security' },
    { label: 'Anchor Action Index docs', url: 'https://docs.anchorbrowser.io/advanced/action-index.md' },
    {
      label: 'Anchor secret values docs',
      url: 'https://docs.anchorbrowser.io/agentic-browser-control/secret-values.md',
    },
  ],
};

/** Browser Use, from its pricing, security, scripts and secrets pages. */
export const BROWSER_USE: Vendor = {
  name: 'Browser Use',
  cells: {
    baa: 'HIPAA on the Enterprise plan; a BAA is not named',
    soc2: 'Type II',
    selfHost: 'On-prem on the Enterprise plan',
    replay: 'No: every rerun of a script starts a model',
    secrets: 'Secret aliases do not guarantee the agent cannot see the value',
    takeover: 'Yes, a live URL to watch or take over',
    price: '$1 credit; pay as you go at $0.06 an hour',
  },
  sources: [
    { label: 'Browser Use pricing', url: 'https://browser-use.com/pricing' },
    { label: 'Browser Use security', url: 'https://browser-use.com/security' },
    { label: 'Browser Use scripts docs', url: 'https://docs.browser-use.com/cloud/agent/scripts' },
    { label: 'Browser Use secrets docs', url: 'https://docs.browser-use.com/cloud/guides/secrets.md' },
  ],
};

/** Kernel, from its pricing, security, audit log and auth pages. */
export const KERNEL: Vendor = {
  name: 'Kernel',
  cells: {
    baa: 'On the enterprise plan',
    soc2: 'Type II',
    selfHost: 'Browser images self-host with Docker (Apache-2.0)',
    audit: 'API request logs, kept 1 year',
    secrets: 'Credentials are never passed to LLMs',
    takeover: 'Yes, live view with control',
    price: '$5 a month of credits free; $30 a month',
  },
  sources: [
    { label: 'Kernel pricing', url: 'https://www.kernel.sh/pricing' },
    { label: 'Kernel security docs', url: 'https://kernel.sh/docs/security' },
    { label: 'Kernel audit logs docs', url: 'https://www.kernel.sh/docs/info/audit-logs.md' },
    { label: 'Kernel managed auth docs', url: 'https://www.kernel.sh/docs/auth/managed-auth.md' },
  ],
};

/** The cloud browser vendors buyers compare Oya with. */
export const INFRA_VENDORS = [BROWSERBASE, STEEL, ANCHOR, BROWSER_USE, KERNEL];

/** What a revenue-cycle team checks before an agent touches a payer portal. */
export const PORTAL_ROWS: Row[] = [
  { key: 'kind', label: 'How you get it' },
  { key: 'baa', label: 'Signs a BAA' },
  { key: 'soc2', label: 'SOC 2' },
  { key: 'selfHost', label: 'Runs in your own infrastructure' },
  { key: 'replay', label: 'Replays with no model' },
  { key: 'code', label: 'Code you keep' },
  { key: 'audit', label: 'Audit trail' },
  { key: 'mfa', label: 'MFA and a person in the loop' },
  { key: 'price', label: 'Public price' },
];

/** Skyvern, from its healthcare, pricing, code caching and GitHub pages. */
export const SKYVERN: Vendor = {
  name: 'Skyvern',
  cells: {
    kind: 'Self-serve platform and API',
    baa: 'Not stated; it says HIPAA compliant',
    soc2: 'Type II',
    selfHost: 'Self-hosts (AGPL-3.0)',
    replay: 'Replays cached Playwright code, with AI on failure',
    code: 'Cached Playwright scripts',
    mfa: 'TOTP, email and SMS codes; live view',
    price: '$29 and $149 a month; enterprise custom',
  },
  sources: [
    { label: 'Skyvern healthcare', url: 'https://www.skyvern.com/healthcare' },
    { label: 'Skyvern pricing', url: 'https://www.skyvern.com/pricing' },
    { label: 'Skyvern code caching docs', url: 'https://www.skyvern.com/docs/developers/features/code-caching' },
    { label: 'Skyvern on GitHub', url: 'https://github.com/Skyvern-AI/skyvern' },
  ],
};

/** CloudCruise, from its pricing, home and docs pages. */
export const CLOUDCRUISE: Vendor = {
  name: 'CloudCruise',
  cells: {
    kind: 'Managed runtime; you build with its builder agent',
    baa: 'Standard BAA from the free plan; custom BAA on Scale',
    replay: 'Deterministic execution; AI builds and maintains workflows',
    audit: 'Video replay, screenshots per action, logs kept 30 days',
    mfa: '2FA; human maintenance fallback on the Accelerate plan',
    price: '$0 plus $3 an automation hour; $700 a month',
  },
  sources: [
    { label: 'CloudCruise pricing', url: 'https://www.cloudcruise.com/pricing' },
    { label: 'CloudCruise', url: 'https://www.cloudcruise.com/' },
    { label: 'CloudCruise docs', url: 'https://docs.cloudcruise.com/' },
  ],
};

/** QuickIntell, from its trust center and payer portal bot pages. */
export const QUICKINTELL: Vendor = {
  name: 'QuickIntell',
  cells: {
    kind: 'Platform and API, with an optional managed service',
    baa: 'BAA available',
    soc2: 'Type II',
    audit: 'Append-only logs and failure screenshots',
    mfa: 'Bots pause for staff MFA; approval gates before writes',
    price: 'Usage-based quote',
  },
  sources: [
    { label: 'QuickIntell trust center', url: 'https://quickintell.com/trust-center' },
    { label: 'QuickIntell payer portal bots', url: 'https://quickintell.com/products/payer-portal-bots' },
  ],
};

/** Champ AI, from its home and security and deployment pages. */
export const CHAMP: Vendor = {
  name: 'Champ AI',
  cells: {
    kind: 'Managed automation',
    baa: 'Not stated; it says built for HIPAA',
    soc2: 'Type II',
    selfHost: 'Your VPC or on-prem',
    audit: 'Immutable audit trails and replayable sessions',
    mfa: 'Approval gates; Slack escalation with the live session',
    price: 'Demo only',
  },
  sources: [
    { label: 'Champ AI', url: 'https://www.champ.ai/' },
    { label: 'Champ AI security and deployment', url: 'https://www.champ.ai/features/security-and-deployment' },
  ],
};

/** The payer portal automation vendors buyers compare Oya with. */
export const PORTAL_VENDORS = [SKYVERN, CLOUDCRUISE, QUICKINTELL, CHAMP];

/** What a person choosing an AI browser to work beside an agent checks. */
export const AI_BROWSER_ROWS: Row[] = [
  { key: 'aiPrice', label: 'Price' },
  { key: 'platforms', label: 'Platforms' },
  { key: 'plan', label: 'Runs on your ChatGPT or Claude plan' },
  { key: 'coding', label: 'Claude Code, Codex or Cursor can drive it' },
  { key: 'tab', label: 'Agent works in its own tab; you take over' },
  { key: 'aiReplay', label: 'A finished task replays with no model' },
  { key: 'passwords', label: 'Passwords kept from the model' },
  { key: 'hipaa', label: 'HIPAA and a BAA' },
];

/** Oya's desktop app, in the site's own words (see AI_BROWSER and CODING_AGENT_BROWSER). */
export const OYA_DESKTOP: Vendor = {
  name: 'Oya',
  cells: {
    aiPrice: 'Free, with 500 agent steps a month',
    platforms: 'macOS, Windows',
    plan: 'Yes: your ChatGPT or Claude plan, or a model API key',
    coding: 'One-click Connect adds it to Claude Code, Codex, Cursor or VS Code over MCP',
    tab: 'Its own tab for each session; take it and hand it back',
    aiReplay: 'Yes: saved as a playbook, Playwright code that replays with no model',
    passwords: 'Sealed and never shown to the model',
    hipaa: 'On Oya Cloud and the enterprise edition, with a BAA',
  },
  sources: [],
};

/** Perplexity's Comet, from its product, pricing and blog pages. */
export const COMET: Vendor = {
  name: 'Comet',
  cells: {
    aiPrice: 'Perplexity Pro $17 or Max $167 a month (annual)',
    platforms: 'Mac, Windows, iOS, Android',
    plan: "Perplexity's choice of models",
    tab: 'A sidecar shows each action, with a stop button',
    passwords: 'Asks before logging in',
    hipaa: 'Says HIPAA compliance for enterprise (2025 announcement)',
  },
  sources: [
    { label: 'Perplexity Comet', url: 'https://www.perplexity.ai/comet' },
    { label: 'Perplexity pricing', url: 'https://www.perplexity.ai/hub/pricing' },
    {
      label: 'Perplexity: mitigating prompt injection in Comet',
      url: 'https://www.perplexity.ai/hub/blog/mitigating-prompt-injection-in-comet',
    },
    {
      label: 'Perplexity: Comet for Enterprise Pro',
      url: 'https://www.perplexity.ai/hub/blog/the-intelligent-business-introducing-comet-for-enterprise-pro',
    },
  ],
};

/** Dia, from its home and security pages. */
export const DIA: Vendor = {
  name: 'Dia',
  cells: {
    aiPrice: 'Free download',
    platforms: 'macOS, Windows',
    passwords: 'Password fields are invisible to the agent',
    hipaa: 'Not HIPAA compliant; no BAA',
  },
  sources: [
    { label: 'Dia', url: 'https://www.diabrowser.com/' },
    { label: 'Dia security', url: 'https://www.diabrowser.com/security' },
  ],
};

/** Claude in Chrome, from Anthropic's help and Claude Code docs. */
export const CLAUDE_IN_CHROME: Vendor = {
  name: 'Claude in Chrome',
  cells: {
    aiPrice: 'Paid Claude plans only',
    platforms: 'Chrome extension on desktop',
    plan: 'Yes: a Claude plan',
    coding: 'Claude Code, with claude --chrome',
    tab: 'Its own tab group, with approval modes',
    aiReplay: 'Saved shortcuts; replay with no model not stated',
    passwords: 'With 1Password (macOS beta), Claude never sees the password',
    hipaa: 'Off by default under HIPAA; the BAA does not cover data sent to the sites it visits',
  },
  sources: [
    {
      label: 'Getting started with Claude in Chrome',
      url: 'https://support.claude.com/en/articles/12012173-getting-started-with-claude-in-chrome',
    },
    { label: 'Claude Code in Chrome docs', url: 'https://code.claude.com/docs/en/chrome' },
  ],
};

/** Aside, from its home, pricing and developer pages. */
export const ASIDE: Vendor = {
  name: 'Aside',
  cells: {
    aiPrice: 'Free with 500 credits a month; Pro $20 a month',
    platforms: 'macOS 15 or later, Windows',
    plan: 'Yes: your ChatGPT or Claude subscription, or an API key',
    coding: 'An MCP server and a CLI',
    aiReplay: 'Deterministic browser steps in its REPL; saved replay not stated',
    passwords: 'Credentials are not exposed to the agent',
  },
  sources: [
    { label: 'Aside', url: 'https://aside.com/' },
    { label: 'Aside pricing', url: 'https://aside.com/pricing' },
    { label: 'Aside for developers', url: 'https://aside.com/developers.md' },
  ],
};

/** ego (lite), from its site and GitHub README. */
export const EGO: Vendor = {
  name: 'ego (lite)',
  cells: {
    aiPrice: 'Free (MIT licence)',
    platforms: 'macOS, Windows',
    plan: 'The model your agent CLI uses',
    coding: 'A skill and a CLI for Claude Code, Codex and Cursor',
    tab: 'Its own Space; take over or stop at any time',
    passwords: 'Stored locally, as Chrome stores them',
  },
  sources: [
    { label: 'ego (lite)', url: 'https://lite.ego.app/' },
    { label: 'ego (lite) on GitHub', url: 'https://github.com/citrolabs/ego-lite' },
  ],
};

/** The AI browsers people compare Oya's desktop app with. */
export const AI_BROWSER_VENDORS = [COMET, DIA, CLAUDE_IN_CHROME, ASIDE, EGO];
