/**
 * The landing page's fixed content: the repository link, the code the hero
 * and the developer section show, and the copy for the sections that are made
 * of lists rather than prose.
 */

/** The public repository. */
export const repository = 'https://github.com/OyadotAI/oya-browser';

/** The install line above the hero snippet. */
export const installCommand = 'npm i @oya-ai/browser';

/** The hero snippet: start a real browser, replay a recorded portal run. */
export const heroExample = `import { Oya } from "@oya-ai/browser";

const browser = await new Oya().browser.start();     // a real browser, already signed in
await browser.goto(PORTAL_URL);

const result = await browser.play("eligibility-check", { memberId });  // no model in the loop`;

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

/** What makes portal work rot, in the order teams hit it. */
export const PORTAL_TRUTHS = [
  {
    n: '01',
    title: 'There is no API.',
    body: 'Payer portals, EHRs, registries and state boards. The last mile is a person in a browser, and your product stops where they start.',
  },
  {
    n: '02',
    title: 'The portal blocks the bot.',
    body: 'Headless Chrome driven over the debugging protocol is the shape anti-bot vendors look for. The task is fine. The browser gives you away.',
  },
  {
    n: '03',
    title: 'The session dies.',
    body: 'SSO, passkeys, a code to a mailbox. Scripted logins break first and break constantly, and a retried password locks the account.',
  },
  {
    n: '04',
    title: 'Every run costs a model call.',
    body: 'An agent that reasons through the same eight steps every night is paying to rediscover them, and drifting each time it does.',
  },
];

/** The features shown beside a clip of the product doing them. */
export const MEDIA = [
  {
    src: '/oya-ask-loop.mp4',
    poster: '/oya-ask-poster.jpg',
    title: 'Do it once. Keep it forever.',
    body: 'Ask in plain language. When the run works, save it: the steps, the selectors and the inputs become a playbook that replays with no model in the loop, and exports as a Playwright module you own.',
    note: 'Recorded in the Oya browser. The boxes are the page as the agent reads it.',
  },
  {
    src: '/oya-browser.mp4',
    poster: '/oya-browser-poster.jpg',
    title: 'One console for every browser.',
    body: 'Start browsers on your own machines or in your own cloud, watch a run as it happens, take the mouse when a portal asks for a code, and read the audit trail afterwards.',
    note: 'The Oya console, self-hosted.',
  },
];

/** Where Oya sits against the browser infrastructure teams already buy. */
export const COMPARE = [
  {
    row: 'What you connect to',
    others: 'Headless Chrome over the debugging protocol',
    oya: 'A real headful browser with the automation inside it',
  },
  {
    row: 'The tenth run of the same task',
    others: 'Another full model pass',
    oya: 'A recorded playbook, replayed with no model',
  },
  {
    row: 'Signing in',
    others: 'Script the login, or paste cookies',
    oya: 'Sign in by hand once; the persona carries cookies and localStorage',
  },
  {
    row: 'Where it runs',
    others: 'Their cloud',
    oya: 'Their cloud, your cloud, or your own machines',
  },
  {
    row: 'What you keep if you leave',
    others: 'The code you wrote against their SDK',
    oya: 'Your playbooks, as Playwright modules',
  },
];

/** The example shown and copied in the developers section. */
export const portalExample = `import { Oya } from "@oya-ai/browser";

const oya = new Oya(); // Set OYA_API_KEY.
const { PORTAL_URL, PORTAL_USERNAME, PORTAL_PASSWORD } = process.env;
if (!PORTAL_URL || !PORTAL_USERNAME || !PORTAL_PASSWORD) {
  throw new Error("Set your test portal URL and credentials.");
}
const secrets = { username: PORTAL_USERNAME, password: PORTAL_PASSWORD };
const playbookName = "portal-request-review";

// First run: let the agent do the task, then store its playbook.
{
  await using browser = await oya.browser.start();
  await browser.goto(PORTAL_URL);
  await browser.ask(
    "If needed, log in with {{username}} and {{password}}. " +
    "Open New Request for {{customerName}}. Stop before submitting.",
    { data: { customerName: "Alex Example" }, secrets },
  );
  await browser.toPlaybook(playbookName);
} // The first browser stops here; the playbook stays saved.

// Later: run the saved playbook on a new browser with new inputs.
await using replay = await oya.browser.start();
await replay.goto(PORTAL_URL);
await replay.play(playbookName, {
  customerName: "Sam Example",
  ...secrets, // Replay remembers which variables are secret.
});`;
