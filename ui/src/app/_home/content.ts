/**
 * The landing page's fixed content: the repository link, the code the hero
 * and the developer section show, and the copy for the sections that are made
 * of lists rather than prose.
 */

/** The public repository. */
export const repository = 'https://github.com/OyadotAI/oya-browser';

/** The community Discord. */
export const discord = 'https://discord.gg/wqSeXJPygn';

/** A call with the founders, booked on Calendly. */
export const foundersCall = 'https://calendly.com/d/dvrm-r65-kkx/oya-founder-call';

/** One field of a portal form in the hero's replay. */
export interface ReplayField {
  /** Its label on the form. */
  label: string;
  /** What the playbook types or picks. */
  value: string;
  /** Whether it is a drop-down. */
  select?: boolean;
}

/** One portal task the hero replays: a form filled line by line as its playbook runs. */
export interface ReplayTask {
  /** The portal address in the window bar. */
  address: string;
  /** The playbook's file name. */
  file: string;
  /** The form's title. */
  title: string;
  /** The form's fields, in the order the playbook fills them. */
  fields: ReplayField[];
  /** The button the playbook presses. */
  button: string;
  /** What the portal answers. */
  result: string;
  /** The playbook: open the page, fill each field, press the button, check the answer. */
  code: string[];
  /** The audit row each line of the playbook writes, one per line, in order. */
  ledger: AuditRow[];
}

/**
 * The replay the hero shows: a saved playbook filing a prior authorization on a payer
 * portal, line by line, with no model, each line writing its audit row. The code is the
 * shape Oya generates; the member, request and hashes are made up.
 */
export const REPLAY: ReplayTask = {
  address: 'portal.payer.example/prior-auth',
  file: 'prior-auth.ts',
  title: 'Prior authorization request',
  fields: [
    { label: 'Member ID', value: 'W2847-1193' },
    { label: 'CPT', value: '72148 · MRI lumbar spine', select: true },
    { label: 'Diagnosis', value: 'M54.50' },
    { label: 'Ordering NPI', value: '1487652309' },
  ],
  button: 'Submit request',
  result: 'Submitted · Ref PA-30418 · Pending review',
  code: [
    'await page.goto(vars["portal"]);',
    'await page.getByLabel("Member ID").fill(vars["memberId"]);',
    'await page.getByLabel("CPT").selectOption(vars["cpt"]);',
    'await page.getByLabel("Diagnosis").fill(vars["icd10"]);',
    'await page.getByLabel("Ordering NPI").fill(vars["npi"]);',
    'await page.getByRole("button", { name: "Submit request" }).click();',
    'await expect(page.getByText("Pending review")).toBeVisible();',
  ],
  ledger: [
    { time: '09:14:02', actor: 'agent · prior-auth', action: 'Opened payer portal', hash: '9f2c41' },
    { time: '09:14:04', actor: 'agent · prior-auth', action: 'Filled member ID', hash: 'b71e08' },
    { time: '09:14:05', actor: 'agent · prior-auth', action: 'Picked CPT 72148', hash: '4d93aa' },
    { time: '09:14:07', actor: 'agent · prior-auth', action: 'Filled diagnosis M54.50', hash: 'e05b7c' },
    { time: '09:14:08', actor: 'agent · prior-auth', action: 'Filled ordering NPI', hash: '31c8f5' },
    { time: '09:14:10', actor: 'agent · prior-auth', action: 'Submitted request', hash: '7a60d2' },
    { time: '09:14:12', actor: 'agent · prior-auth', action: 'Read reference PA-30418', hash: 'c2d9e4' },
  ],
};

/** The same prior authorization started from your stack, one snippet per way in; MCP first, since most teams start from an agent. */
export const FROM_AGENT = [
  {
    id: 'mcp',
    tab: 'mcp',
    code: [
      '# Claude Code, Codex, Cursor or any MCP client',
      'claude mcp add --transport http oya https://oyabrowser.com/mcp/pool \\',
      '  --header "Authorization: Bearer $OYA_API_KEY"',
      '',
      '> Run the prior-auth playbook for member W2847-1193',
    ],
  },
  {
    id: 'sdk',
    tab: 'sdk.ts',
    code: [
      'import { Oya } from "@oya-ai/browser";',
      '',
      'const browser = await new Oya().browser.start();',
      'await browser.play("prior-auth", { memberId: "W2847-1193" });',
      'await browser.stop();',
    ],
  },
  {
    id: 'playwright',
    tab: 'playwright.ts',
    code: [
      'import { chromium } from "playwright-core";',
      'import { Oya } from "@oya-ai/browser";',
      '',
      'const browser = await new Oya().browser.start();',
      'const cdp = await chromium.connectOverCDP(browser.cdpUrl!);',
      'await cdp.contexts()[0].pages()[0].goto("https://portal.payer.example/prior-auth");',
    ],
  },
];

/** The one throughput claim, said once so the home page and the prior-auth page cannot drift apart. */
export const THROUGHPUT = 'Customers report 10 to 30 times the throughput of the same work by hand.';

/** How a task becomes code, in the three steps a team takes. */
export const STEPS = [
  {
    n: '01',
    title: 'Run it once',
    body: 'An agent files the prior auth on the payer portal, signed in as your staff, while a person can watch.',
  },
  {
    n: '02',
    title: 'Save it as code',
    body: 'The run becomes a playbook: Playwright code you can read, with the member and codes as variables.',
  },
  {
    n: '03',
    title: 'Replay with no model',
    body: `One SDK call runs it again with new inputs. ${THROUGHPUT}`,
  },
];

/** One measured agent run against its replay: the README sample (one run, Gemini 3.8 Flash). */
export const MEASURED = { before: '152.7s', after: '14.2s' };

/**
 * What Oya may say about compliance, one claim each (owner decision, 2026-11-08):
 * HIPAA compliant, BAAs signed (Google Cloud, Vertex AI and Sentry, 2026-10-07), and
 * the SOC 2 Type II audit in progress through Rippling. Never "SOC 2 certified" or a
 * SOC 2 mark until the report is issued.
 */
export const COMPLIANCE_CLAIMS = ['HIPAA compliant', 'We sign BAAs', 'SOC 2 Type II audit in progress'];

/** The compliance claims as one line, for the footer. */
export const COMPLIANCE_LINE = COMPLIANCE_CLAIMS.join(' · ');

/** Who backs Oya and who builds it, named as text: no third-party logos. */
export const BACKERS = ['AWS Partner', 'AWS Startups', 'Google for Startups', 'Built by engineers from Google'];

/** One row of an audit trail, as the hero ledger and the audit section draw it. */
export interface AuditRow {
  /** When, as a clock time. */
  time: string;
  /** Who acted: an agent or a person, and their role. */
  actor: string;
  /** What they did. */
  action: string;
  /** The first characters of the row's hash, which links it to the row before. */
  hash: string;
}

/** An example audit trail: fictional, shaped like the rows server/src/platform/audit-chain.ts writes. */
export const AUDIT_ROWS: AuditRow[] = [
  { time: '09:14:02', actor: 'agent · eligibility-bot', action: 'Signed in to payer portal', hash: '9f2c41' },
  { time: '09:14:09', actor: 'agent · eligibility-bot', action: 'Filled member ID (password sealed)', hash: 'b71e08' },
  { time: '09:14:15', actor: 'agent · eligibility-bot', action: 'Read eligibility: active', hash: '4d93aa' },
  { time: '09:15:40', actor: 'person · S. Patel, operator', action: 'Took control of session', hash: 'e05b7c' },
  { time: '09:16:02', actor: 'person · S. Patel, operator', action: 'Returned control to agent', hash: '31c8f5' },
  { time: '09:16:05', actor: 'agent · eligibility-bot', action: 'Saved playbook eligibility-check', hash: '7a60d2' },
];

/** The five questions a regulated team asks before wiring Oya in. */
export const FAQ = [
  {
    q: 'Do you sign a BAA?',
    a: 'Yes. We sign BAAs. Our SOC 2 Type II audit is in progress.',
  },
  {
    q: 'Can it touch PHI?',
    a: 'Self-hosted, pages, recordings and audit records stay in your network. Recordings expire after 7 days by default, and the audit retention floor is 365 days. Recordings are not yet redacted for PHI in the community edition; turn recording off for sessions that show PHI.',
  },
  {
    q: 'Can we host it ourselves?',
    a: 'Yes. The control plane is source-available and runs on Docker, ECS, GCP or Kubernetes, so HIPAA workloads stay in your own infrastructure. Every playbook also exports as Playwright code you keep.',
  },
  {
    q: 'What happens when a portal changes?',
    a: 'The replay stops at the step that moved, the agent finishes the task from there, and its fix becomes the playbook, so the next run is model-free again.',
  },
  {
    q: 'How much does it cost?',
    a: 'The desktop app is free, and so are 500 agent steps a month. Oya Cloud: Free gives 1 cloud browser hour; Developer is $20 a month for 100 hours, 1 GB of proxy and 5,000 agent steps; Startup is $99 a month for 500 hours, 5 GB and 50,000 steps. Past that, $0.10 or $0.08 a browser hour, $8 a GB and $0.002 a step. Self-hosting is free up to 5 cloud browsers at once. For more, write to sales@getoya.ai.',
  },
];

/** One field of a portal form a workflow fills, for the finished forms the workflow tabs show. */
const field = (label: string, value: string, select = false): ReplayField => ({ label, value, select });

/** One revenue-cycle workflow Oya runs on a portal: its tab, its two lines, and the finished form it shows. */
export interface Workflow {
  /** Stable id, for the tab, its panel and its screen. */
  id: string;
  /** The tab's label: a verb and an object. */
  tab: string;
  /** What it does, in two sentences. */
  body: string;
  /** The portal form as the agent leaves it, finished. */
  form: Pick<ReplayTask, 'title' | 'fields' | 'button' | 'result'>;
}

/** The revenue-cycle work staff do by hand on portals, as Oya runs it (founder, 2026-11-08). */
export const WORKFLOWS: Workflow[] = [
  {
    id: 'prior-auth',
    tab: 'File prior auths',
    body: 'Submit and track prior authorizations on payer portals that have no API. Staff time per case drops from 22 to 26 minutes to under 2.',
    form: { title: 'Prior authorization request', fields: REPLAY.fields, button: REPLAY.button, result: REPLAY.result },
  },
  {
    id: 'eligibility',
    tab: 'Check eligibility',
    body: 'Verify coverage, copays and deductibles for everyone on tomorrow’s schedule, tonight. The answer is written back to the chart.',
    form: {
      title: 'Eligibility and benefits',
      fields: [
        field('Member ID', 'W2847-1193'),
        field('Date of birth', '03/14/1968'),
        field('Service type', '30 · Health benefit plan', true),
      ],
      button: 'Check eligibility',
      result: 'Coverage active · PPO Gold · Copay $30',
    },
  },
  {
    id: 'claims',
    tab: 'Pull claim status',
    body: 'Read claim status and remittance detail from each payer on a schedule. Every read lands on the audit trail.',
    form: {
      title: 'Claim status inquiry',
      fields: [
        field('Claim number', 'CLM-4471902'),
        field('Date of service', '09/02/2026'),
        field('Billed amount', '$1,240.00'),
      ],
      button: 'Search claims',
      result: 'Paid · $986.40 · Check 0931177',
    },
  },
  {
    id: 'denials',
    tab: 'Work denials',
    body: 'Collect the denial reason, gather the records, and file the appeal on the payer’s own form before the deadline.',
    form: {
      title: 'Appeal submission',
      fields: [
        field('Claim number', 'CLM-4471902'),
        field('Denial reason', 'CO-197 · Precert absent', true),
        field('Attachment', 'clinical-notes.pdf'),
      ],
      button: 'Submit appeal',
      result: 'Appeal received · Ref AP-88214',
    },
  },
  {
    id: 'enrollment',
    tab: 'Keep enrollment current',
    body: 'Keep provider enrollment and payer rosters current without retyping them. Attestations and renewals run on their dates.',
    form: {
      title: 'Provider roster update',
      fields: [
        field('Provider NPI', '1487652309'),
        field('Practice location', 'Oak Street Clinic'),
        field('Effective date', '12/01/2026'),
      ],
      button: 'Update roster',
      result: 'Roster updated · Attested',
    },
  },
  {
    id: 'registries',
    tab: 'Search registries',
    body: 'Look up immunizations, licences and sanctions in state registries that only have a login page. Results are logged with their source.',
    form: {
      title: 'Immunization record search',
      fields: [field('Last name', 'Rivera'), field('Date of birth', '07/02/2015'), field('ZIP', '94110')],
      button: 'Search',
      result: '3 records · Up to date',
    },
  },
];

/** The questions a visitor asks, in the order they ask them; the FAQ shows every answer open. */
export const QUESTIONS = FAQ;

/** What the audit section lists beside the trail: three plain facts a buyer can check. */
export const AUDIT_FACTS = [
  'Rows are hash-chained; the database refuses edits and deletes.',
  'A person can watch any session live, take over and hand it back.',
  'Audit records are kept for 365 days at least.',
];

/** The control-plane diagram: agents into Oya, Oya out to the portals; two-letter marks, no borrowed logos. */
export const DIAGRAM = {
  layout: 'hub',
  alt: 'Claude Code, Codex and your own code send work through Oya to payer portals, EHRs, clearinghouses and state registries.',
  center: { label: 'Oya', icon: 'text:Oy' },
  spokes: [
    { label: 'Claude Code', icon: 'text:Cl' },
    { label: 'Codex', icon: 'text:Cx' },
    { label: 'Your code', icon: 'text:TS' },
    { label: 'Payer portal', icon: 'text:Pa' },
    { label: 'EHR', icon: 'text:EH' },
    { label: 'Clearinghouse', icon: 'text:Ch' },
    { label: 'State registry', icon: 'text:Rg' },
  ],
  motion: { cycle: 3.2 },
};
