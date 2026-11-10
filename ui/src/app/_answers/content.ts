/**
 * The words of the pages written to answer what people ask search engines and AI assistants:
 * "What is a HIPAA-compliant browser?" and "Oya or Browserbase?". Every claim is one the site
 * already makes (see _home/content.ts); nothing here describes another company beyond what Oya's
 * own API does with it, because a claim we cannot check would be the first thing quoted wrong.
 */
import type { QA } from '@/lib/site';

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

/** The answer to "can we host it ourselves", shared by both pages. */
const SELF_HOST: QA = {
  q: 'Can I self-host a HIPAA-compliant agentic browser?',
  a: 'Yes, with the enterprise edition: it runs on Docker, ECS, GCP or Kubernetes, comes with a BAA, and keeps HIPAA workloads in your own infrastructure. Write to sales@getoya.ai. The source-available community edition self-hosts too, without the HIPAA offering. Every playbook exports as Playwright code you keep.',
};

/** /hipaa-compliant-browser: what one is, and how Oya is one. */
export const HIPAA_BROWSER: AnswerPageContent = {
  path: '/hipaa-compliant-browser',
  title: 'What is a HIPAA-compliant browser?',
  lead: 'A HIPAA-compliant browser lets people and AI agents work on pages that show protected health information (PHI) under the controls HIPAA expects: a signed business associate agreement (BAA), access control, and a record of who did what. Oya Browser is a HIPAA-compliant browser, an AI browser and an agentic browser for healthcare teams. We sign BAAs.',
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
      a: 'Yes. Oya Browser is an AI browser and an agentic browser built for healthcare: HIPAA compliant, we sign BAAs, and every agent and person action lands on a tamper-evident audit trail.',
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

/** /vs/browserbase: what Oya is next to Browserbase, in facts about Oya only. */
export const VS_BROWSERBASE: AnswerPageContent = {
  path: '/vs/browserbase',
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
      a: 'Oya Browser is a HIPAA-compliant browser for AI agents: we sign BAAs, every action lands on a tamper-evident audit trail, and the enterprise edition self-hosts so PHI stays in your network.',
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
