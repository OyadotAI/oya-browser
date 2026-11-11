/**
 * The docs' parts, in page order. The HTML page renders one <section> per
 * part, and each part also has a Markdown twin at /docs/<slug>.md cut from
 * that section, so this list is the one place both read from. A part with a
 * `topic` also has its own HTML page at /docs/<slug>, so a search engine has
 * one page per subject to rank rather than only the single long /docs.
 */

/** What a part's own page is called: what a person searching for the subject would type. */
export interface DocTopic {
  /** The page's title and h1. Longer than the part's `title`, which is only a label. */
  heading: string;
  /** The meta description and the visible first paragraph. */
  description: string;
}

/** One part of the docs. */
export interface DocPage {
  /** Its name in /docs/<slug>.md and on its <section data-doc-page>. */
  slug: string;
  /** Its title, the first line of its Markdown. */
  title: string;
  /** Its own page at /docs/<slug>, when it has one. */
  topic?: DocTopic;
}

/** Every part, in the order the page shows them. */
export const DOC_PAGES: DocPage[] = [
  {
    slug: 'getting-started',
    title: 'Get started',
    topic: {
      heading: 'Quickstart, API keys and desktop sign-in',
      description:
        'Get an API key, run a task once with an agent and replay it with no model, in about three minutes. Then sign in on the desktop and connect a browser.',
    },
  },
  {
    slug: 'playbooks',
    title: 'Playbooks',
    topic: {
      heading: 'Playbooks: record once, replay with no model',
      description:
        'A playbook is a task the agent did once, kept as Playwright code. Variables and secrets, healing when a page changes, signing in on replay, export and background runs.',
    },
  },
  {
    slug: 'sdk',
    title: 'SDK',
    topic: {
      heading: 'TypeScript SDK for browser agents',
      description:
        '@oya-ai/browser, the TypeScript SDK with no runtime dependencies: start a browser, analyze the page, act on elements by ID, or bring Playwright over the CDP URL.',
    },
  },
  {
    slug: 'cli',
    title: 'CLI',
    topic: {
      heading: 'CLI: drive a browser from your terminal',
      description:
        'The oya command line: save an API key, choose a model and a browser provider, start a browser and drive it from a terminal.',
    },
  },
  {
    slug: 'mcp',
    title: 'MCP setup',
    topic: {
      heading: 'MCP setup for Claude Code, Cursor and Codex',
      description:
        'Connect Oya Browser to Claude Code, Cursor, Codex, claude.ai, Claude Desktop or the Oya desktop app as an MCP server, through one endpoint that starts browsers itself.',
    },
  },
  {
    slug: 'mcp-tools',
    title: 'MCP tools',
    topic: {
      heading: 'MCP tools reference',
      description:
        'Every Oya Browser MCP tool with its parameters: analyze_page, navigate, click, type, press_key, screenshot, scroll, tabs, wait and the rest.',
    },
  },
  {
    slug: 'identity',
    title: 'Identity',
    topic: {
      heading: 'Personas, CAPTCHA and MFA',
      description:
        'A persona is one identity: a fingerprint, a cookie jar and a proxy, bound together. How to create one, rotate between personas, solve CAPTCHAs and complete MFA.',
    },
  },
  {
    slug: 'anonymity',
    title: 'Stealth and anonymity',
    topic: {
      heading: 'Stealth, fingerprints and proxies',
      description:
        'How Oya Browser gives each profile its own fingerprint, proxy and cookie store, what the stealth layer does, and how to choose a persona.',
    },
  },
  {
    slug: 'self-hosting',
    title: 'Self-hosting',
    topic: {
      heading: 'Self-hosting: the whole stack in your network',
      description:
        'Run the whole Oya stack in your own network with git, Docker and Node 20 or later: install it, connect a browser, and the settings that tune it.',
    },
  },
  // No topic page yet: this section says things about other vendors that Oya cannot check, which is fine in a
  // reference but not on a page built to rank. It gets one when the section is rewritten.
  { slug: 'control-plane', title: 'Control plane' },
  {
    slug: 'dashboard',
    title: 'Dashboard',
    topic: {
      heading: 'Dashboard: providers, live view and settings',
      description:
        'The Oya console: add a provider, drive a browser from the live view, connect to one that is already running, stop a browser, and change settings.',
    },
  },
  {
    slug: 'api',
    title: 'REST and WebSocket API',
    topic: {
      heading: 'REST and WebSocket API reference',
      description:
        'Every Oya Browser REST route under /api, the command API with examples, and the WebSocket protocol browsers connect over.',
    },
  },
];

/** A part that has its own page. */
export type DocTopicPage = DocPage & { /** Its page. */ topic: DocTopic };

/** The parts that have their own page at /docs/<slug>, in page order. Derived, so /docs still renders every part. */
export const DOC_TOPICS = DOC_PAGES.filter((page): page is DocTopicPage => page.topic !== undefined);

/** The part a slug names, if any. */
export function docPage(slug: string): DocPage | undefined {
  return DOC_PAGES.find((page) => page.slug === slug);
}

/** The part a slug names, when it has its own page. */
export function docTopic(slug: string): DocTopicPage | undefined {
  return DOC_TOPICS.find((page) => page.slug === slug);
}
