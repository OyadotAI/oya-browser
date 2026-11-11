/**
 * The words of the pages written to answer what people ask search engines and AI assistants:
 * "What is a HIPAA-compliant browser?", "an AI browser?", "a browser for healthcare?", HIPAA
 * browser automation, prior authorization, and "Oya or Browserbase, Steel, Anchor or Browser Use?"
 * (one page per vendor Oya's own API drives, each from a different side of Oya so they are not one
 * page with the name swapped). HIPAA and BAAs are always
 * scoped to Oya Cloud and the enterprise edition, never the community edition. Every claim is one the site
 * already makes (see _home/content.ts); nothing here describes another company beyond what Oya's
 * own API does with it, because a claim we cannot check would be the first thing quoted wrong.
 */
import type { QA } from '@/lib/site';
import { THROUGHPUT } from '../_home/content';

/** One section of an answer page: a heading, a paragraph, and optionally a list. */
export interface AnswerSection {
  /** The section's heading. */
  heading: string;
  /** Its paragraph. */
  body: string;
  /** Points listed under the paragraph, when there are any. */
  items?: string[];
}

/** An answer page: its address, its title, the answer first, then sections and questions. */
export interface AnswerPageContent {
  /** Its path, for the canonical link and the sitemap. */
  path: string;
  /** Its short name in the site footer, which links every answer page so crawlers find them. */
  label: string;
  /** The page title and heading, phrased as the question people ask. */
  title: string;
  /** The answer, in the first paragraph: what an assistant quotes. */
  lead: string;
  /** What follows the answer. */
  sections: AnswerSection[];
  /** The questions shown on the page and repeated as FAQPage structured data. */
  faq: QA[];
}

/** How an agent and a person act on a page under HIPAA's controls, in Oya's own words. */
const OYA_CONTROLS = [
  'We sign BAAs. Our SOC 2 Type II audit is in progress.',
  'Every action, by an agent or a person, is a hash-chained audit row the database refuses to edit or delete, kept for 365 days at least.',
  'Oya Cloud, or the enterprise edition in your own infrastructure (Docker, ECS, GCP or Kubernetes), so PHI can stay in your network.',
  'A person can watch any session live, take over and hand it back.',
  'Browsers reach only the hosts you allow.',
];

/** The answer to "can we host it ourselves", shared by the pages that are asked it. */
const SELF_HOST: QA = {
  q: 'Can I self-host a HIPAA-compliant agentic browser?',
  a: 'Yes, with the enterprise edition: it runs on Docker, ECS, GCP or Kubernetes, comes with a BAA, and keeps HIPAA workloads in your own infrastructure. Write to sales@getoya.ai. The source-available community edition self-hosts too, without the HIPAA offering. Every playbook exports as Playwright code you keep.',
};

/** /hipaa-compliant-browser: what one is, and how Oya is one. */
export const HIPAA_BROWSER: AnswerPageContent = {
  path: '/hipaa-compliant-browser',
  label: 'HIPAA browser',
  title: 'What is a HIPAA-compliant browser?',
  lead: 'A HIPAA-compliant browser lets people and AI agents work on pages that show protected health information (PHI) under the controls HIPAA expects: a signed business associate agreement (BAA), access control, and a record of who did what. Oya Browser is a HIPAA-compliant browser on Oya Cloud and the enterprise edition, where we sign BAAs. It is also an AI browser and an agentic browser for healthcare teams.',
  sections: [
    {
      heading: 'What a HIPAA-compliant browser needs',
      body: 'When a browser, or an agent driving one, opens a payer portal, an EHR or a registry, it sees PHI. HIPAA then asks for these things.',
      items: [
        'A business associate agreement with the company that runs the browsers.',
        'Access control: only the people and agents you allow, and only the systems you allow them to reach.',
        'An audit trail of every action, that nobody can quietly change.',
        'Control over where the data lives: your own network, if you need it.',
        'A person who can watch and take over.',
      ],
    },
    {
      heading: 'How Oya Browser does each',
      body: 'Oya runs browser agents on payer portals, EHRs and registries, and keeps the controls in the browser itself.',
      items: OYA_CONTROLS,
    },
    {
      heading: 'An AI browser and an agentic browser',
      body: 'The Oya desktop app for macOS and Windows is an AI browser: you browse, and an agent can take a tab while you can take it back. Oya Cloud and self-hosted Oya are an agentic browser for teams: agents run many browsers at once, each one on the audit trail.',
    },
  ],
  faq: [
    {
      q: 'Is there a HIPAA-compliant AI browser?',
      a: 'Yes. On Oya Cloud and the enterprise edition, Oya Browser is an agentic browser built for healthcare: HIPAA compliant, we sign BAAs, and every agent and person action lands on a tamper-evident audit trail.',
    },
    {
      q: 'Can AI agents access PHI in a browser?',
      a: 'Under a BAA and with the right controls, yes. Oya records every action on a hash-chained audit trail, lets browsers reach only the hosts you allow, and lets a person take over any session. Self-hosted, pages, recordings and audit records stay in your network. Recordings are not yet redacted for PHI in the community edition; turn recording off for sessions that show PHI.',
    },
    { q: 'Does Oya sign a BAA?', a: 'Yes. We sign BAAs. Write to sales@getoya.ai.' },
    { q: 'Is Oya SOC 2 certified?', a: 'Our SOC 2 Type II audit is in progress.' },
    SELF_HOST,
  ],
};

/** Sealed credentials, as the README states it. */
const SEALED = 'Stored passwords and TOTP seeds are sealed and never shown to the model.';

/** /vs/browserbase: what Oya is next to Browserbase, in facts about Oya only. */
export const VS_BROWSERBASE: AnswerPageContent = {
  path: '/vs/browserbase',
  label: 'Oya vs Browserbase',
  title: 'Oya vs Browserbase',
  lead: 'Choosing a browser for AI agents? Oya is built for healthcare and regulated work: on Oya Cloud and the enterprise edition it is HIPAA compliant, we sign BAAs, and every action by an agent or a person lands on a tamper-evident audit trail. Oya can also run browsers on Browserbase, behind the same API.',
  sections: [
    {
      heading: 'Where Oya fits',
      body: 'Payer portals, EHRs and registries: the systems revenue-cycle and operations teams still work by hand. An agent does the task once in plain language, Oya saves it as a playbook (Playwright code), and the playbook replays with no model in the loop.',
    },
    {
      heading: 'What Oya gives a regulated team',
      body: 'On Oya Cloud and the enterprise edition, the controls live in the browser.',
      items: OYA_CONTROLS,
    },
    {
      heading: 'Use Browserbase through Oya',
      body: 'One Oya API drives Oya Cloud, Browserbase, Steel, Anchor, Browser Use or your own Chrome. Keep Browserbase as a provider behind the same API, and move a workload to Oya Cloud or the enterprise edition whenever you choose. The controls and BAA above cover Oya Cloud and the enterprise edition.',
    },
  ],
  faq: [
    {
      q: 'Is Oya a HIPAA-compliant alternative to Browserbase?',
      a: 'On Oya Cloud and the enterprise edition, Oya Browser is a HIPAA-compliant browser for AI agents: we sign BAAs, every action lands on a tamper-evident audit trail, and the enterprise edition self-hosts so PHI stays in your network.',
    },
    {
      q: 'Can I use Oya with Browserbase?',
      a: "Yes. Browserbase is one of the providers Oya's API drives, next to Oya Cloud, Steel, Anchor, Browser Use and your own Chrome.",
    },
    {
      q: 'Can I keep my Playwright code if I move to Oya?',
      a: 'Yes. Oya gives a CDP URL that Playwright connects to, and every playbook exports as a readable Playwright module you own.',
    },
    SELF_HOST,
    {
      q: 'Is there a desktop AI browser too?',
      a: 'Yes. The Oya desktop app for macOS and Windows is free: an AI browser you use yourself, where an agent can take a tab and you can take it back.',
    },
  ],
};

/** /vs/steel: what Oya is next to Steel, in facts about Oya only: where it runs. */
export const VS_STEEL: AnswerPageContent = {
  path: '/vs/steel',
  label: 'Oya vs Steel',
  title: 'Oya vs Steel',
  lead: 'Comparing browsers for AI agents? Oya Browser runs agents on Oya Cloud, in your own infrastructure or on your desktop, and saves each finished task as a playbook: Playwright code that replays with no model. On Oya Cloud and the enterprise edition it is HIPAA compliant and we sign BAAs. Oya can also run browsers on Steel, behind the same API.',
  sections: [
    {
      heading: 'Where Oya runs',
      body: 'Pick where the browsers live, and keep one API across all of them.',
      items: [
        'Oya Cloud: browsers we run for you.',
        'The enterprise edition: the whole stack in your own infrastructure, on Docker, ECS, GCP or Kubernetes, with a BAA.',
        'The community edition: source-available and self-hosted, free up to 5 cloud browsers at once, without the HIPAA offering.',
        'The desktop app for macOS and Windows: free, and signed in as you.',
      ],
    },
    {
      heading: 'What Oya gives a regulated team',
      body: 'On Oya Cloud and the enterprise edition, the controls live in the browser.',
      items: OYA_CONTROLS,
    },
    {
      heading: 'Use Steel through Oya',
      body: "Steel is one of the providers Oya's API drives, next to Oya Cloud, Browserbase, Anchor, Browser Use and your own Chrome. Add it in the console under Control, Providers with your Steel API key, and set its routing priority. When a connection to one provider fails, Oya sends the session to the next one in priority order. The controls and BAA above cover Oya Cloud and the enterprise edition.",
    },
  ],
  faq: [
    {
      q: 'Is Oya a self-hosted alternative to Steel?',
      a: 'Oya self-hosts in two editions. The source-available community edition runs in your network and is free up to 5 cloud browsers at once. The enterprise edition runs on Docker, ECS, GCP or Kubernetes, comes with a BAA, and keeps HIPAA workloads in your own infrastructure. Write to sales@getoya.ai.',
    },
    {
      q: 'Can I use Oya with Steel?',
      a: "Yes. Steel is one of the providers Oya's API drives, next to Oya Cloud, Browserbase, Anchor, Browser Use and your own Chrome. Add it with your Steel API key and your code keeps calling the Oya API.",
    },
    {
      q: 'What happens when a provider is down?',
      a: 'When a connection to one provider fails, Oya rests that route for a while and sends the session to the next provider in priority order.',
    },
    {
      q: 'Does Oya work with Playwright and Puppeteer?',
      a: 'Yes. Each Oya browser gives a CDP URL. Point Playwright, Puppeteer, Stagehand or browser-use at it. Every playbook also exports as a readable Playwright module you own.',
    },
    {
      q: 'Is Oya HIPAA compliant?',
      a: 'On Oya Cloud and the enterprise edition, yes: we sign BAAs, and every action by an agent or a person lands on a tamper-evident audit trail. Our SOC 2 Type II audit is in progress.',
    },
  ],
};

/** /vs/anchor: what Oya is next to Anchor, in facts about Oya only: agents that work signed in. */
export const VS_ANCHOR: AnswerPageContent = {
  path: '/vs/anchor',
  label: 'Oya vs Anchor',
  title: 'Oya vs Anchor',
  lead: 'Looking at browsers for AI agents that have to sign in? In Oya Browser an agent works signed in as your staff: stored passwords and TOTP seeds are sealed and never shown to the model, and a person can take over any session and hand it back. On Oya Cloud and the enterprise edition it is HIPAA compliant and we sign BAAs. Oya can also run browsers on Anchor, behind the same API.',
  sections: [
    {
      heading: 'Signed in, without showing the model a password',
      body: 'Most of the work worth automating sits behind a login, so Oya treats the login as part of the browser.',
      items: [
        SEALED,
        'A persona is one identity: a fingerprint, a cookie jar and a proxy, bound together and stable for its life.',
        'Sign in once in the desktop app to the sites your agents need, and those cookies move to the browsers Oya runs, which keep the same fingerprint as that identity.',
        'When a sign-in needs a person, such as a text-message code, staff take over the session and hand it back.',
      ],
    },
    {
      heading: 'A record of every step',
      body: 'On Oya Cloud and the enterprise edition, the controls live in the browser.',
      items: OYA_CONTROLS,
    },
    {
      heading: 'Use Anchor through Oya',
      body: 'One Oya API drives Oya Cloud, Anchor, Browserbase, Steel, Browser Use or your own Chrome. Keep Anchor as a provider behind that API, set its routing priority and session capacity next to your other providers, and move a workload without changing your code. The controls and BAA above cover Oya Cloud and the enterprise edition.',
    },
  ],
  faq: [
    {
      q: 'Is Oya an alternative to Anchor for agents that sign in?',
      a: 'Oya Browser runs agents signed in as your staff. Stored passwords and TOTP seeds are sealed and never shown to the model, and a person can take over any session. On Oya Cloud and the enterprise edition every action lands on a tamper-evident audit trail, and we sign BAAs.',
    },
    {
      q: 'Can I use Oya with Anchor?',
      a: "Yes. Anchor is one of the providers Oya's API drives, next to Oya Cloud, Browserbase, Steel, Browser Use and your own Chrome.",
    },
    { q: 'Does the agent see my passwords?', a: `No. ${SEALED}` },
    {
      q: 'What about two-factor codes?',
      a: 'A persona can hold a TOTP seed, sealed like a password, and Oya completes the code step with it. When a sign-in needs a person, such as a text-message code, staff can take over the session and hand it back.',
    },
    {
      q: 'Is Oya HIPAA compliant?',
      a: 'On Oya Cloud and the enterprise edition, yes: we sign BAAs. Our SOC 2 Type II audit is in progress.',
    },
    SELF_HOST,
  ],
};

/** /vs/browser-use: what Oya is next to Browser Use, in facts about Oya only: a run becomes code that replays with no model. */
export const VS_BROWSER_USE: AnswerPageContent = {
  path: '/vs/browser-use',
  label: 'Oya vs Browser Use',
  title: 'Oya vs Browser Use',
  lead: 'Weighing Oya against Browser Use for AI agents in a browser? In Oya Browser an agent does a task once in plain language, and Oya saves it as a playbook: Playwright code that replays with no model in the loop. On Oya Cloud and the enterprise edition it is HIPAA compliant and we sign BAAs. Oya can also run browsers on Browser Use, behind the same API.',
  sections: [
    {
      heading: 'Run it once with an agent, then replay it with no model',
      body: 'An agent does the task once in plain language. Oya saves it as a playbook (Playwright code, with the inputs as variables), and the playbook replays with no model calls. When a page changes, the replay stops at the step that moved, the agent finishes from there, and its fix becomes the playbook.',
    },
    {
      heading: 'Bring the agent you already have',
      body: 'Oya is the browser. The agent can be yours.',
      items: [
        'MCP: Claude Code, Codex, Cursor or any MCP client.',
        'The Ask panel in the desktop app, on the ChatGPT or Claude plan you already pay for, or a model API key.',
        'The TypeScript SDK, @oya-ai/browser.',
        'Playwright, Puppeteer, Stagehand or browser-use, over the CDP URL each browser gives.',
      ],
    },
    {
      heading: 'For regulated work',
      body: 'On Oya Cloud and the enterprise edition, the controls live in the browser.',
      items: OYA_CONTROLS,
    },
    {
      heading: 'Use Browser Use through Oya',
      body: "Browser Use is one of the providers Oya's API drives, next to Oya Cloud, Browserbase, Steel, Anchor and your own Chrome: add it with your Browser Use API key and Oya starts browsers there behind the same API. It works the other way round too: point the browser-use library at an Oya browser's CDP URL. The controls and BAA above cover Oya Cloud and the enterprise edition.",
    },
  ],
  faq: [
    {
      q: 'Is Oya an alternative to Browser Use?',
      a: 'Oya Browser is a browser for AI agents that takes a task in plain language, or takes your own agent over MCP, the SDK or a CDP URL. What a run leaves behind is a playbook: Playwright code that replays with no model. On Oya Cloud and the enterprise edition it is HIPAA compliant and we sign BAAs.',
    },
    {
      q: 'Can I use the browser-use library with Oya?',
      a: 'Yes. Each Oya browser gives a CDP URL. Point browser-use, Playwright, Puppeteer or Stagehand at it.',
    },
    {
      q: 'Can Oya run browsers on Browser Use?',
      a: "Yes. Browser Use is one of the providers Oya's API drives, next to Oya Cloud, Browserbase, Steel, Anchor and your own Chrome.",
    },
    {
      q: 'Does every run need a model?',
      a: 'No. The first run does. A saved playbook replays with no model calls, and the agent is called again only when a page has changed, to finish the task from the step that moved.',
    },
    {
      q: 'Is there a free way to try it?',
      a: 'Yes. The desktop app for macOS and Windows is free, and so are 500 agent steps a month.',
    },
    SELF_HOST,
  ],
};

/** /ai-browser: what an AI browser is, and Oya's free desktop app. */
export const AI_BROWSER: AnswerPageContent = {
  path: '/ai-browser',
  label: 'AI browser',
  title: 'What is an AI browser?',
  lead: 'An AI browser is a web browser with an AI agent that can open pages, fill forms and finish tasks in a tab while you watch. Oya Browser is a free AI browser for macOS and Windows: type a task in the Ask panel or connect Claude Code, Codex or Cursor, and take the tab back at any time.',
  sections: [
    {
      heading: 'What an AI browser does',
      body: 'You stay in an ordinary browser, with your own tabs and logins. The agent works in a tab of its own.',
      items: [
        'It opens pages, clicks, types and fills forms in its tab.',
        'It works signed in as you, with the logins already in your browser.',
        'It waits when you take the tab, and carries on when you hand it back.',
        'A task it finished can be saved as a playbook that replays with no model.',
      ],
    },
    {
      heading: 'Two ways to start',
      body: 'The first screen asks which one you want.',
      items: [
        'The Ask panel: connect the ChatGPT or Claude plan you already pay for, or paste a model API key, and type the task.',
        'Your own agent: one click on Connect in the window footer adds Oya to Claude Code, Codex, Cursor or VS Code as an MCP server.',
      ],
    },
    {
      heading: 'An AI browser and an agentic browser',
      body: 'The desktop app is an AI browser for one person. When a team needs many browsers at once, Oya Cloud and the enterprise edition run them as an agentic browser, with every action on a tamper-evident audit trail. HIPAA compliance and a signed BAA come with Oya Cloud and the enterprise edition.',
    },
  ],
  faq: [
    {
      q: 'Is Oya Browser a free AI browser?',
      a: 'Yes. The desktop app for macOS and Windows is free, and so are 500 agent steps a month.',
    },
    {
      q: 'Which AI can drive Oya Browser?',
      a: 'Your ChatGPT or Claude plan through the Ask panel, a model API key, or a coding agent such as Claude Code, Codex, Cursor or VS Code, connected with one click as an MCP server.',
    },
    {
      q: 'Can I take over from the AI agent?',
      a: 'Yes. Take the tab and the agent waits; hand it back and it carries on.',
    },
    { q: 'Does the agent see my passwords?', a: `No. ${SEALED}` },
    {
      q: 'Is the Oya AI browser HIPAA compliant?',
      a: 'HIPAA compliance and a signed BAA come with Oya Cloud and the enterprise edition. For work on PHI, run your agents there. Write to sales@getoya.ai.',
    },
    {
      q: 'What is the difference between an AI browser and an agentic browser?',
      a: 'An AI browser is one you use yourself, with an agent beside you in a tab. An agentic browser is one agents run on their own, many at once, in the cloud or self-hosted. Oya is both: the desktop app, and Oya Cloud and the enterprise edition.',
    },
  ],
};

/** /browser-for-healthcare: what a browser for healthcare does, with the BAA questions folded in. */
export const HEALTHCARE_BROWSER: AnswerPageContent = {
  path: '/browser-for-healthcare',
  label: 'For healthcare',
  title: 'What should a browser for healthcare do?',
  lead: 'A browser for healthcare lets staff and AI agents work on payer portals, EHRs and registries that show protected health information (PHI), under a signed business associate agreement (BAA) and a record of who did what. Oya Browser is a browser for healthcare: on Oya Cloud and the enterprise edition it is HIPAA compliant, and we sign BAAs.',
  sections: [
    {
      heading: 'The work it is for',
      body: 'Revenue-cycle and operations teams still do this by hand, on portals that have no API.',
      items: [
        'Filing and tracking prior authorizations on payer portals.',
        'Checking eligibility, copays and deductibles before the visit.',
        'Pulling claim status and remittance detail from each payer.',
        'Working denials and filing appeals on the payer form.',
        'Keeping provider enrollment and payer rosters current.',
        'Searching state registries for immunizations, licences and sanctions.',
      ],
    },
    {
      heading: 'What it has to prove',
      body: 'On Oya Cloud and the enterprise edition, these controls live in the browser.',
      items: OYA_CONTROLS,
    },
    {
      heading: 'A BAA for browser automation',
      body: 'We sign BAAs for Oya Cloud and the enterprise edition. Write to sales@getoya.ai with how many browsers you need and whether PHI must stay in your own infrastructure. The enterprise edition runs there, on Docker, ECS, GCP or Kubernetes.',
    },
  ],
  faq: [
    {
      q: 'Will you sign a BAA for browser automation?',
      a: 'Yes. We sign BAAs for Oya Cloud and the enterprise edition. Write to sales@getoya.ai.',
    },
    {
      q: 'Which editions of Oya are HIPAA compliant?',
      a: 'Oya Cloud and the enterprise edition. The source-available community edition self-hosts without the HIPAA offering.',
    },
    { q: 'Is Oya SOC 2 certified?', a: 'Our SOC 2 Type II audit is in progress.' },
    {
      q: 'Can staff watch what the agent does?',
      a: 'Yes. A person can watch any session live, take over and hand it back. Every action by an agent or a person is a row on a hash-chained audit trail, kept for 365 days at least.',
    },
    {
      q: 'Who is behind Oya?',
      a: 'Oya is an AWS Partner, in AWS Startups and Google for Startups, and built by engineers from Google.',
    },
    SELF_HOST,
  ],
};

/** /hipaa-browser-automation: HIPAA-compliant browser automation and AI browser agents. */
export const HIPAA_AUTOMATION: AnswerPageContent = {
  path: '/hipaa-browser-automation',
  label: 'HIPAA automation',
  title: 'What is HIPAA-compliant browser automation?',
  lead: 'HIPAA-compliant browser automation is software, often an AI browser agent, that works on web pages showing PHI under a signed BAA, with access control and an audit trail of every action. Oya runs HIPAA-compliant browser automation on Oya Cloud and the enterprise edition: we sign BAAs, and every agent and person action lands on a tamper-evident audit trail.',
  sections: [
    {
      heading: 'What it needs',
      body: 'An agent on a payer portal or an EHR sees PHI, so HIPAA asks for these things.',
      items: [
        'A BAA with the company that runs the browsers and the agents.',
        'Agents that reach only the systems you allow.',
        'Credentials the model never sees.',
        'An audit trail nobody can quietly change.',
        'A person who can watch and take over.',
      ],
    },
    {
      heading: 'How Oya does each',
      body: 'On Oya Cloud and the enterprise edition, the controls live in the browser.',
      items: [...OYA_CONTROLS, SEALED],
    },
    {
      heading: 'Run it once, then replay it with no model',
      body: 'An agent does the task once in plain language. Oya saves it as a playbook (Playwright code, with the patient details and codes as variables), and the playbook replays with no model calls. When a portal changes, the replay stops at the step that moved, the agent finishes from there, and its fix becomes the playbook.',
    },
    {
      heading: 'From the tools you already use',
      body: 'Start an agent or a playbook from your own stack.',
      items: [
        'MCP: Claude Code, Codex, Cursor or any MCP client.',
        'The TypeScript SDK, @oya-ai/browser.',
        'Playwright, over the CDP URL each browser gives.',
      ],
    },
  ],
  faq: [
    {
      q: 'Is there a HIPAA-compliant AI browser agent?',
      a: 'Yes. On Oya Cloud and the enterprise edition, Oya runs AI browser agents under a BAA, with every action on a hash-chained audit trail and a person able to take over any session.',
    },
    {
      q: 'Does PHI go to the AI model?',
      a: `A model sees the page while an agent works a task. A saved playbook replays with no model at all. ${SEALED} Each project can allow only the model providers you choose.`,
    },
    { q: 'Does Oya sign a BAA?', a: 'Yes, for Oya Cloud and the enterprise edition. Write to sales@getoya.ai.' },
    {
      q: 'Can I keep my Playwright code?',
      a: 'Yes. Oya gives a CDP URL that Playwright connects to, and every playbook exports as a readable Playwright module you own.',
    },
    SELF_HOST,
  ],
};

/** /prior-authorization-automation: filing prior auths on payer portals with an agent, then a playbook. */
export const PRIOR_AUTH: AnswerPageContent = {
  path: '/prior-authorization-automation',
  label: 'Prior auth',
  title: 'How do you automate prior authorization on payer portals?',
  lead: 'Prior authorization automation files and tracks prior auths on payer portals that have no API, the way staff do by hand. With Oya, an AI agent files one prior auth on the portal, signed in as your staff, and Oya saves it as a playbook that files the next ones with no model. On Oya Cloud and the enterprise edition this runs under a BAA.',
  sections: [
    {
      heading: 'How it works',
      body: 'Three steps, the same on every payer portal.',
      items: [
        'Run it once: an agent files the prior auth on the payer portal while a person can watch.',
        'Save it as code: the run becomes a playbook, Playwright code you can read, with the member ID, CPT code, diagnosis and ordering NPI as variables.',
        'Replay with no model: one SDK call or one MCP request runs it again with new inputs.',
      ],
    },
    {
      heading: 'When the portal changes',
      body: 'The replay stops at the step that moved, the agent finishes the request from there, and its fix becomes the playbook, so the next run makes no model calls again.',
    },
    {
      heading: "Under HIPAA's controls",
      body: 'On Oya Cloud and the enterprise edition, the controls live in the browser.',
      items: OYA_CONTROLS,
    },
  ],
  faq: [
    {
      q: 'Can AI submit prior authorizations?',
      a: 'Yes. An Oya agent fills in the payer portal form and submits it, signed in as your staff, and every step is a row on the audit trail. A person can watch the session and take over at any point.',
    },
    {
      q: 'Does the payer need an API?',
      a: 'No. Oya works the portal the way staff do, in a real browser, so it fits payers that have no API.',
    },
    {
      q: 'Can it track requests already filed?',
      a: 'Yes. A playbook can read a request status on the portal on a schedule, and every read lands on the audit trail.',
    },
    {
      q: 'What about portal logins and two-factor codes?',
      a: `${SEALED} When a sign-in needs a person, such as a text-message code, staff can take over the session and hand it back.`,
    },
    { q: 'How much faster is it?', a: THROUGHPUT },
    {
      q: 'Is prior authorization automation with Oya HIPAA compliant?',
      a: 'On Oya Cloud and the enterprise edition, yes: Oya is HIPAA compliant and we sign BAAs. Write to sales@getoya.ai.',
    },
  ],
};

/** Every answer page, for the footer links and the tests: one list, so a new page cannot be left unlinked. */
export const ANSWER_PAGES = [
  HIPAA_BROWSER,
  HEALTHCARE_BROWSER,
  AI_BROWSER,
  HIPAA_AUTOMATION,
  PRIOR_AUTH,
  VS_BROWSERBASE,
  VS_STEEL,
  VS_ANCHOR,
  VS_BROWSER_USE,
];
