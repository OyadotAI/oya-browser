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

/** The install line above the hero snippet. */
export const installCommand = 'npm i @oya-ai/browser';

/** The hero snippet: start a real browser, replay a recorded portal run. */
export const heroExample = `import { Oya } from "@oya-ai/browser";

const browser = await new Oya().browser.start();     // a real browser, already signed in
await browser.goto(PORTAL_URL);

const result = await browser.play("eligibility-check", { memberId });  // no model in the loop`;

/**
 * The replay the hero shows: a saved playbook filling a payer portal's
 * eligibility form, line by line, with no model. The code is the shape Oya
 * generates; the member is made up.
 */
export const SHOWCASE = {
  address: 'portal.payer.example/eligibility',
  file: 'eligibility-check.ts',
  title: 'Eligibility and benefits',
  fields: [
    { label: 'Member ID', value: 'W2847-1193' },
    { label: 'Date of birth', value: '03/14/1968' },
    { label: 'Provider NPI', value: '1487652309' },
    { label: 'Service type', value: '30 · Health benefit plan', select: true },
  ],
  button: 'Check eligibility',
  result: 'Coverage active · PPO Gold',
  /** The comment the playbook ends on once it has run. */
  done: '// 7 steps replayed. No model, no tokens, nothing to review.',
  code: [
    'await page.goto(vars["portal"]);',
    'await page.getByLabel("Member ID").fill(vars["memberId"]);',
    'await page.getByLabel("Date of birth").fill(vars["dob"]);',
    'await page.getByLabel("Provider NPI").fill(vars["npi"]);',
    'await page.getByLabel("Service type").selectOption("30");',
    'await page.getByRole("button", { name: "Check eligibility" }).click();',
    'await expect(page.getByText("Coverage active")).toBeVisible();',
  ],
};

/** The size of the portal icon each lane of the hero figure ends with. */
export const BOT_WALL_ICON = 19;

/** The two attempts the hero figure animates. */
export const BOT_WALL_LANES = [
  {
    kind: 'blocked' as const,
    who: 'Everyone else',
    how: 'headless Chrome over CDP',
    result: 'Blocked. Verify you are human',
  },
  {
    kind: 'through' as const,
    who: 'Oya',
    how: 'a real browser, signed in',
    result: 'Eligibility: active',
  },
];

/** The recorded product walkthrough, played from Loom above the clips. */
export const WALKTHROUGH = {
  /** The player, started on the press of the poster, without Loom's own title bar. */
  playing:
    'https://www.loom.com/embed/7947a9a863634f84a4be7b71933c4b7d?autoplay=true&hide_owner=true&hide_share=true&hide_title=true&hideEmbedTopBar=true&default_speed=true',
  title: 'Oya Browser, faster scalable automation without CDP',
  /** How long it runs, shown on the poster. */
  length: '5 min',
};

/** The numbers under the hero, as Oya reports them. */
export const PROOF = [
  { value: 95, suffix: '%+', label: 'task success on the portal runs Oya has measured' },
  { value: 0, suffix: '', label: 'model calls on a replayed run: pure compute' },
  { value: 0, suffix: '%', label: 'flagged as a bot on CreepJS' },
  { value: 10, suffix: '', label: 'browsers started in under five seconds, and scaling on' },
];

/** The three problems Oya was built to solve, each with its answer. */
export const PROBLEMS = [
  {
    n: '01',
    title: 'Blocked as a bot.',
    problem: 'Headless Chrome driven over the debugging protocol is exactly what anti-bot systems look for.',
    answer: 'Oya is the browser. No CDP, no headless Chrome: it passes iphey and CreepJS like a person would.',
  },
  {
    n: '02',
    title: 'Paying the model every run.',
    problem: 'An agent that reasons through the same steps every night pays for them every night, and drifts.',
    answer:
      'Record the task once. Every run after that is pure compute, with no model, and it heals itself when a page moves.',
  },
  {
    n: '03',
    title: 'No real browsers at scale.',
    problem: 'Agents need a real, signed-in browser each, and they need hundreds of them on demand.',
    answer:
      'Go from zero to thousands of real browsers, each on its own profile with your team’s cookies and sessions.',
  },
];

/** The browsers Oya drives, named under the proof numbers. */
export const PROVIDERS = ['Oya Cloud', 'Browserbase', 'Browser Use', 'Steel', 'Anchor', 'Your own Chrome'];

/** How a portal task becomes a playbook, in the three steps a team takes. */
export const STEPS = [
  {
    n: '01',
    title: 'Ask.',
    body: 'Describe the task in plain language, in the console, the SDK or your own agent. Oya does it in a real browser, signed in like your staff.',
  },
  {
    n: '02',
    title: 'Save.',
    body: 'Keep the run as a playbook. The values you gave become variables, the steps become Playwright code you own, and free-text answers stay with your model.',
  },
  {
    n: '03',
    title: 'Replay.',
    body: 'Run it with new inputs and no model in the loop. It signs in by itself, and when a portal changes, the agent fixes the step and the playbook heals.',
  },
];

/** Why the browser being Oya's own matters, one card each. */
export const BENEFITS = [
  {
    icon: 'fingerprint',
    title: 'Not flagged as a bot.',
    body: 'Chrome’s own emulation sets the device. 0% on CreepJS.',
  },
  {
    icon: 'key',
    title: 'Sign in once.',
    body: 'Log in by hand. Every browser after that starts signed in, cookies and localStorage both.',
  },
  {
    icon: 'lock',
    title: 'Signs itself back in.',
    body: 'A replay that meets a login uses the profile’s stored credentials and second factor, then carries on.',
  },
  {
    icon: 'pen',
    title: 'Writes the free text fresh.',
    body: 'Comments and question answers are written by your model on every run, never replayed word for word.',
  },
  {
    icon: 'shield',
    title: 'Proves what happened.',
    body: 'A hash-chained audit trail. Allow-list the hosts a browser may reach.',
  },
  {
    icon: 'move',
    title: 'Runs in your cloud.',
    body: 'Source-available. Self-host on Docker, ECS, GCP or Kubernetes for HIPAA and SOC 2 workloads.',
  },
] as const;

/** Questions teams ask before they wire Oya into a workflow. */
export const FAQ = [
  {
    q: 'Is Oya a real browser, or Chrome driven from outside?',
    a: 'A real headful browser with the automation inside it. Sites see an ordinary Chrome, not a harness attached over the debugging protocol.',
  },
  {
    q: 'Where does it run?',
    a: 'In Oya Cloud, in your own cloud with the control plane self-hosted, on your team’s machines with the desktop app, or on Browserbase, Browser Use, Steel and Anchor.',
  },
  {
    q: 'What happens when a portal changes?',
    a: 'The replay stops at the step that moved, the agent finishes the task from there, and its fix becomes the playbook, so the next run is model-free again.',
  },
  {
    q: 'Does the model see our passwords?',
    a: 'No. Secrets are typed through placeholders and redacted from everything the model reads, and they never enter a saved playbook.',
  },
  {
    q: 'Can we host it ourselves?',
    a: 'Yes. The control plane is source-available and runs on Docker, ECS, GCP or Kubernetes, so HIPAA and SOC 2 workloads stay in your own infrastructure. Every playbook also exports as Playwright code you keep.',
  },
  {
    q: 'How much does it cost?',
    a: 'The desktop app is free, and so are 500 agent steps a month. Oya Cloud: Free gives 1 cloud browser hour; Developer is $20 a month for 100 hours, 1 GB of proxy and 5,000 agent steps; Startup is $99 a month for 500 hours, 5 GB and 50,000 steps. Past that, $0.10 or $0.08 a browser hour, $8 a GB and $0.002 a step. Self-hosting is free up to 5 cloud browsers at once. For more, write to sales@getoya.ai.',
  },
];

/** The vendors a buyer weighs Oya against, as the comparison table's columns. */
export const RIVALS = ['Browser Use', 'Anchor', 'Browserbase'];

/**
 * Oya against the browser-agent vendors a buyer is also weighing, on the questions
 * a regulated team asks. From each vendor's public site, docs and pricing page.
 */
export const COMPARE = [
  {
    row: 'What the portal sees',
    oya: 'A real headful browser',
    rivals: ['Chromium over CDP', 'A stealth Chromium fork', 'Cloud Chromium over CDP'],
  },
  {
    row: 'Signed in as your team',
    oya: 'Your staff’s real sessions, synced as profiles',
    rivals: ['Browser profiles', 'Managed login', 'Stored contexts'],
  },
  {
    row: 'Replays without a model',
    oya: 'Yes, with no model at all',
    rivals: ['Yes, checked by an LLM each run', 'Yes', 'Yes'],
  },
  {
    row: 'Runs in your own cloud',
    oya: 'Yes, source-available',
    rivals: ['The library only', 'Enterprise plan only', 'No'],
  },
  {
    row: 'HIPAA, data in your network',
    oya: 'Self-host on any plan',
    rivals: ['Enterprise plan', 'Enterprise plan', 'Scale plan'],
  },
];
