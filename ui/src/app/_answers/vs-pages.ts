/**
 * Comparison pages for local MCP browser servers (Playwright MCP, Chrome DevTools MCP),
 * developer browser/extraction stacks (Firecrawl, Stagehand) and payer-portal automation
 * (Skyvern). Every competitor claim comes from dated primary pages in compare.ts; HIPAA
 * and BAAs are scoped strictly to Oya Cloud and the enterprise edition.
 */
import { BUILD_ACTION, DEMO_ACTION, DOWNLOAD_ACTION } from './story-actions';
import { COMPARISON_CLOSING, COMPARISON_REQUIREMENTS, PUBLISHED_DIFFERENCES, SELF_HOST_FAQ } from './review-chapters';
import {
  CHECKED,
  CHROME_DEVTOOLS_MCP,
  compareTable,
  DEV_STACK_ROWS,
  FIRECRAWL,
  MCP_ROWS,
  OYA,
  PLAYWRIGHT_MCP,
  PORTAL_ROWS,
  SKYVERN,
  STAGEHAND,
} from './compare';
import type { AnswerPageContent } from './content';

/** /vs/playwright-mcp: Oya next to Microsoft Playwright MCP for coding agents. */
export const VS_PLAYWRIGHT_MCP: AnswerPageContent = {
  path: '/vs/playwright-mcp',
  label: 'Oya vs Playwright MCP',
  title: 'Oya vs Playwright MCP: giving coding agents a browser',
  lead: 'Comparing Oya Browser and Playwright MCP for Claude Code, Codex or Cursor? Playwright MCP is an Apache-2.0 MCP server that drives browsers via accessibility snapshots and notes it is not a security boundary. Oya gives each coding-agent session its own tab with live takeover, sealed passwords, saved Playwright playbooks that replay with no model, and optional cloud or self-hosted HIPAA mode under a BAA on Oya Cloud and the enterprise edition.',
  hero: {
    headline: 'Oya vs Playwright MCP: choose the browser behind your coding agent.',
    paragraphs: [
      'When Claude Code, Codex, Cursor or VS Code needs to work in a real browser, decide whether you want a stateless local MCP wrapper or a browser built for signed-in sessions, live takeover and repeatable playbooks.',
    ],
    action: DOWNLOAD_ACTION,
  },
  sections: [
    {
      heading: 'Start with how your coding agent uses the browser.',
      body: 'List whether the agent only tests local pages during development, or signs in to real web apps where you need to watch, step in and rerun the finished task later.',
    },
    {
      heading: 'Check session isolation, secrets and replay.',
      body: 'Compare how credentials stay away from the model, what happens when a site requires a person to step in, and whether a completed run becomes reusable code without paying for a model every time.',
      layout: 'controls',
    },
    {
      ...PUBLISHED_DIFFERENCES,
      table: compareTable('Oya and Playwright MCP, from each project\u2019s own pages', MCP_ROWS, [
        OYA,
        PLAYWRIGHT_MCP,
      ]),
      fit: {
        oya: {
          heading: 'Choose Oya when',
          points: [
            'You want one-click MCP setup with a separate agent tab you can watch, take over and hand back.',
            'You need sealed passwords and TOTP seeds hidden from the model, plus saved Playwright playbooks that replay with no model.',
            'Your workflow may move from desktop to Oya Cloud or self-hosted enterprise infrastructure with an audit trail or a HIPAA BAA.',
          ],
        },
        other: {
          heading: 'Choose Playwright MCP when',
          points: [
            'You want a lightweight, Apache-2.0 npm package (npx @playwright/mcp@latest) with no desktop app to install.',
            'You need cross-browser testing across Chromium, Firefox and WebKit or --codegen test snippets in TypeScript, Python, Java or C#.',
            'You only automate local dev or non-sensitive sites where a security boundary, audit trail and BAA are not required.',
          ],
        },
      },
    },
    {
      heading: 'Playwright code stays portable either way.',
      body: 'Oya does not route browsers through Playwright MCP, yet both speak standard Playwright. Every Oya playbook exports as a readable Playwright module, and any Playwright script can connect directly to an Oya browser over its CDP URL.',
    },
    COMPARISON_CLOSING,
  ],
  faq: [
    {
      q: 'What is the difference between Oya Browser and Playwright MCP?',
      a: 'Playwright MCP is an open-source MCP server from Microsoft that lets agents drive Playwright browsers via accessibility snapshots and explicitly notes it is not a security boundary. Oya Browser is a desktop, cloud and self-hosted browser for AI agents that adds a separate agent tab with live takeover, sealed passwords and TOTP seeds, saved Playwright playbooks that replay with no model, and an audit trail with a Business Associate Agreement (BAA) on Oya Cloud and the enterprise edition.',
    },
    {
      q: 'Can I use Oya with Claude Code, Codex, Cursor or VS Code?',
      a: 'Yes. Open the Oya desktop app and click Connect next to Claude Code, Codex, Cursor or VS Code to register the local oya-browser MCP server with no Oya account required. Any MCP client can also connect to Oya Cloud.',
    },
    {
      q: 'Can I keep using my existing Playwright tests with Oya?',
      a: 'Yes. Every Oya browser exposes a CDP URL that Playwright connects to directly, and every Oya playbook exports as standard Playwright code you own.',
    },
    {
      q: 'Does Playwright MCP or Oya hide passwords from the AI model?',
      a: 'Playwright MCP documents --secrets for dotenv values and --redact-url-params for URL parameters. Oya seals stored passwords and TOTP seeds so they are never shown to the model.',
    },
    {
      q: 'Is Oya Browser free like Playwright MCP?',
      a: 'Playwright MCP is free under Apache-2.0. The Oya desktop app for macOS and Windows is free with 500 agent steps a month, and the source-available community edition self-hosts free up to 5 cloud browsers at once. Paid plans cover Oya Cloud and the enterprise edition.',
    },
  ],
  sources: { checked: CHECKED, links: PLAYWRIGHT_MCP.sources },
};

/** /vs/chrome-devtools-mcp: Oya next to Google Chrome DevTools MCP for coding agents. */
export const VS_CHROME_DEVTOOLS_MCP: AnswerPageContent = {
  path: '/vs/chrome-devtools-mcp',
  label: 'Oya vs Chrome DevTools MCP',
  title: 'Oya vs Chrome DevTools MCP for AI coding agents',
  lead: 'Comparing Oya Browser and Chrome DevTools MCP? Chrome DevTools MCP connects coding agents to Google Chrome via Puppeteer and DevTools for debugging console logs, network requests and performance traces, while warning against exposing sensitive data. Oya gives agents a separate tab with live takeover, sealed credentials, saved Playwright playbooks and HIPAA mode with a BAA on Oya Cloud and the enterprise edition.',
  hero: {
    headline: 'Oya vs Chrome DevTools MCP: debugging local pages or running agent workflows.',
    paragraphs: [
      'Both connect Claude Code, Codex, Cursor and other MCP clients to a real browser. Compare whether you need Chrome DevTools inspection during development or isolated, repeatable browser automation with credential and audit controls.',
    ],
    action: DOWNLOAD_ACTION,
  },
  sections: [
    {
      heading: 'Separate frontend debugging from signed-in automation.',
      body: 'Chrome DevTools MCP is built to let a coding agent inspect your own app through DevTools. Oya is built to let agents complete web tasks in a dedicated tab, keep credentials sealed and replay finished workflows with no model.',
    },
    {
      heading: 'Review data exposure, telemetry and repeatability.',
      body: 'Check what browser data reaches the MCP client, whether telemetry or external performance APIs are active by default, and how you will rerun a finished workflow in production.',
      layout: 'controls',
    },
    {
      ...PUBLISHED_DIFFERENCES,
      table: compareTable('Oya and Chrome DevTools MCP, from each project\u2019s own pages', MCP_ROWS, [
        OYA,
        CHROME_DEVTOOLS_MCP,
      ]),
      fit: {
        oya: {
          heading: 'Choose Oya when',
          points: [
            'Your agent signs in to real web apps and needs sealed passwords, TOTP handling and a separate tab you can take over.',
            'You want finished agent runs saved as Playwright playbooks that replay deterministically with no model.',
            'You need cloud execution, self-hosting, an append-only audit trail or HIPAA compliance with a BAA on Oya Cloud or the enterprise edition.',
          ],
        },
        other: {
          heading: 'Choose Chrome DevTools MCP when',
          points: [
            'Your coding agent is debugging a web app and needs Chrome DevTools console, network, Lighthouse or performance traces.',
            'You want an Apache-2.0 npm server (npx -y chrome-devtools-mcp@latest) wired straight into your existing Chrome stable install.',
            'You are working on non-sensitive local or staging sites where DevTools exposure and opt-out telemetry flags fit your policy.',
          ],
        },
      },
    },
    {
      heading: 'Use each tool for the job it was built for.',
      body: 'Oya does not route browsers through Chrome DevTools MCP. Teams often keep Chrome DevTools MCP for local frontend profiling and use Oya when coding agents or production workflows need isolated tabs, sealed logins, replayable playbooks or regulated deployments.',
    },
    COMPARISON_CLOSING,
  ],
  faq: [
    {
      q: 'When should I use Oya instead of Chrome DevTools MCP?',
      a: 'Use Chrome DevTools MCP when your coding agent needs DevTools inspection such as console logs, network requests, Lighthouse audits or performance traces. Use Oya when the agent needs to work signed in across real websites with sealed passwords, live human takeover, repeatable Playwright playbooks, or cloud and self-hosted execution.',
    },
    {
      q: 'What privacy and telemetry notes does Chrome DevTools MCP publish?',
      a: 'Its README notes that full browser and DevTools content is exposed to MCP clients and advises against sharing sensitive or personal information. Usage statistics are enabled by default (disable with --no-usage-statistics), and performance tools may send trace URLs to the Google CrUX API unless --no-performance-crux is set.',
    },
    {
      q: 'Does Oya work with Puppeteer or Playwright?',
      a: 'Yes. Every Oya browser gives a CDP URL that Puppeteer or Playwright can connect to, and every finished Oya task saves as a readable Playwright playbook you own.',
    },
    {
      q: 'Can I use Oya locally for free?',
      a: 'Yes. The Oya desktop app for macOS and Windows is free with 500 agent steps a month, and one-click Connect adds oya-browser to Claude Code, Codex, Cursor or VS Code with no Oya account required.',
    },
    {
      q: 'Is Oya HIPAA compliant?',
      a: 'On Oya Cloud and the enterprise edition, yes: Oya is HIPAA compliant and we sign Business Associate Agreements (BAAs). The desktop app and community edition are not covered by the HIPAA offering.',
    },
  ],
  sources: { checked: CHECKED, links: CHROME_DEVTOOLS_MCP.sources },
};

/** /vs/firecrawl: Oya next to Firecrawl web data API and Browser Sandbox. */
export const VS_FIRECRAWL: AnswerPageContent = {
  path: '/vs/firecrawl',
  label: 'Oya vs Firecrawl',
  title: 'Oya vs Firecrawl: web data API vs signed-in agent browser',
  lead: 'Comparing Oya Browser and Firecrawl? Firecrawl turns websites into LLM-ready data via scrape, crawl, map and search APIs plus an /interact Browser Sandbox and SOC 2 Type II. Oya is an agent browser and control plane built for signed-in workflows, sealed passwords and TOTP, live takeover, Playwright playbooks that replay with no model, and HIPAA compliance with a BAA on Oya Cloud and the enterprise edition.',
  hero: {
    headline: 'Oya vs Firecrawl: extract public web data or run signed-in workflows.',
    paragraphs: [
      'Both offer browser interaction and MCP support, yet they start from different jobs. Compare whether your pipeline scrapes public pages into structured data or signs in to portals that require sealed credentials, human takeover and deterministic replay.',
    ],
    action: BUILD_ACTION,
  },
  sections: [
    {
      heading: 'Start with whether the page is public or behind a staff login.',
      body: 'Public web collection and authenticated portal execution carry different requirements. Map out login handling, two-factor steps, repeat execution cost and compliance obligations before picking one stack.',
    },
    COMPARISON_REQUIREMENTS,
    {
      ...PUBLISHED_DIFFERENCES,
      table: compareTable('Oya and Firecrawl, from each vendor\u2019s own pages', DEV_STACK_ROWS, [OYA, FIRECRAWL]),
      fit: {
        oya: {
          heading: 'Choose Oya when',
          points: [
            'Agents must sign in to portals with sealed passwords and TOTP seeds that the model never sees.',
            'You want finished browser workflows exported as standalone Playwright playbooks that replay with no model.',
            'You need an append-only audit trail, self-hosting in your own network, or HIPAA compliance with a signed BAA on Oya Cloud or the enterprise edition.',
          ],
        },
        other: {
          heading: 'Choose Firecrawl when',
          points: [
            'Your primary job is scraping, crawling, mapping or searching public websites into clean Markdown or JSON.',
            'You want credit-based API pricing starting with a free tier (1,000 credits) and low-entry paid plans ($16/mo annual Hobby).',
            'You want SOC 2 Type II or Enterprise Zero Data Retention for web extraction without needing a signed HIPAA BAA.',
          ],
        },
      },
    },
    {
      heading: 'How teams pair them cleanly.',
      body: 'Oya does not route browsers through Firecrawl. Teams that need both public web research and authenticated portal actions often use Firecrawl for bulk scraping and Oya for signed-in, audited browser workflows and HIPAA-covered workloads.',
    },
    COMPARISON_CLOSING,
  ],
  faq: [
    {
      q: 'What is the main difference between Oya Browser and Firecrawl?',
      a: 'Firecrawl is a web data API built for scraping, crawling, mapping and searching websites, with an /interact Browser Sandbox for scripted or prompted sessions. Oya Browser is a desktop, cloud and self-hosted browser built for signed-in agent tasks: sealed passwords and TOTP, live human takeover, standalone Playwright playbooks that replay with no model, an append-only audit trail, and HIPAA compliance with a BAA on Oya Cloud and the enterprise edition.',
    },
    {
      q: 'Does Firecrawl state HIPAA compliance or a BAA?',
      a: `Firecrawl states SOC 2 Type II on all plans and Zero Data Retention on Enterprise; HIPAA compliance and a BAA were not stated on the Firecrawl pricing and documentation pages reviewed on ${CHECKED}. Oya signs BAAs for HIPAA mode on Oya Cloud and the enterprise edition.`,
    },
    {
      q: 'Can I self-host Oya or Firecrawl?',
      a: 'Firecrawl provides a self-hostable API under AGPL-3.0 and a firecrawl-local Rust binary. Oya self-hosts as the source-available community edition (free up to 5 browsers) or the enterprise edition on Docker, ECS, GCP or Kubernetes with a BAA.',
    },
    {
      q: 'Does Oya export Playwright code I can run elsewhere?',
      a: 'Yes. Every finished Oya playbook exports as a readable Playwright module that runs on any Chromium browser over CDP, with no model in the loop.',
    },
    SELF_HOST_FAQ,
  ],
  sources: { checked: CHECKED, links: FIRECRAWL.sources },
};

/** /vs/stagehand: Oya next to Stagehand SDK, including Stagehand v3 CDP vs v4 extension accuracy. */
export const VS_STAGEHAND: AnswerPageContent = {
  path: '/vs/stagehand',
  label: 'Oya vs Stagehand',
  title: 'Oya vs Stagehand: AI browser SDK vs full browser control plane',
  lead: 'Comparing Oya Browser and Stagehand? Stagehand is Browserbase\u2019s open-source MIT SDK (TypeScript, Python, Go) with act, extract and observe primitives and local or Browserbase caching. Oya is a browser and control plane that exports finished tasks as standalone Playwright playbooks, seals passwords and TOTP, adds live takeover and an audit trail, and signs HIPAA BAAs on Oya Cloud and the enterprise edition. Stagehand v3 can also drive Oya browsers over CDP.',
  hero: {
    headline: 'Oya vs Stagehand: an agent SDK, or the browser and playbook layer beneath it.',
    paragraphs: [
      'Stagehand gives developers act, extract and observe primitives in code. Compare whether you want an open-source SDK alone, or a browser platform with standalone Playwright playbooks, sealed credentials, audit logs and HIPAA deployment.',
    ],
    action: BUILD_ACTION,
  },
  sections: [
    {
      heading: 'Start with what happens after your first script works.',
      body: 'Getting an agent to click through a form once is the easy part. Decide how you will cache or replay runs that touch credentials and personal data, where the browser fleet runs, and what your security team will audit.',
    },
    COMPARISON_REQUIREMENTS,
    {
      ...PUBLISHED_DIFFERENCES,
      table: compareTable('Oya and Stagehand, from each project\u2019s own pages', DEV_STACK_ROWS, [OYA, STAGEHAND]),
      fit: {
        oya: {
          heading: 'Choose Oya when',
          points: [
            'You want finished runs exported as standalone Playwright playbooks that replay with no model, including on authenticated workflows.',
            'You need sealed passwords and TOTP, live human takeover, and an append-only audit trail built into the browser platform.',
            'You need self-hosted infrastructure or HIPAA mode with a signed BAA on Oya Cloud or the enterprise edition.',
          ],
        },
        other: {
          heading: 'Choose Stagehand when',
          points: [
            'You want an open-source MIT SDK in TypeScript, Python or Go built around act, extract and observe primitives.',
            'You already host your own browsers or use Browserbase and want lightweight code-first control over each model step.',
            'Your caching needs fit Local Cache or Browserbase Cache on non-personalized pages without credentials or personal data.',
          ],
        },
      },
    },
    {
      heading: 'Run Stagehand v3 on Oya browsers when you want both.',
      body: 'Oya does not route browsers through Stagehand, because Stagehand is an SDK rather than a browser host. Stagehand v3 connects over CDP, so you can point Stagehand v3 at an Oya browser\u2019s CDP URL (cdpUrl) to combine Stagehand code with Oya\u2019s self-hosting, audit trail and HIPAA-mode browsers. Stagehand v4 runs inside its own browser extension, which Oya browsers do not load yet.',
    },
    COMPARISON_CLOSING,
  ],
  faq: [
    {
      q: 'Can I use Stagehand with Oya Browser?',
      a: 'Yes, with Stagehand v3. Every Oya browser exposes a CDP URL that Stagehand v3 connects to via cdpUrl, running inside Oya Cloud or self-hosted browsers. Stagehand v4 runs inside its own browser extension, which Oya browsers do not load yet.',
    },
    {
      q: 'How do Oya playbooks differ from Stagehand caching?',
      a: 'Stagehand offers Local Cache (cacheDir) and Browserbase Cache, and its caching docs recommend Browserbase Cache only for non-personalized pages without credentials or personal data. Oya saves finished tasks as readable Playwright playbooks that replay with no model on your own inputs, and the agent repairs only a step that moved.',
    },
    {
      q: 'Does Stagehand include a browser fleet, live view or HIPAA BAA?',
      a: 'Stagehand itself is an open-source MIT SDK. Hosted browser fleets, live view, SOC 2 Type II and a BAA on the Scale plan come from Browserbase rather than the standalone SDK. Oya provides the browser, live takeover, audit trail and HIPAA BAA on Oya Cloud and the enterprise edition.',
    },
    {
      q: 'How do Stagehand and Oya handle login credentials?',
      a: 'Stagehand\u2019s observe() pattern returns selectors so your code fills credentials from environment variables without passing them to the model. Oya seals stored passwords and TOTP seeds inside the platform so the model never sees them, and lets staff take over live for text-message codes.',
    },
    SELF_HOST_FAQ,
  ],
  sources: { checked: CHECKED, links: STAGEHAND.sources },
};

/** /vs/skyvern: Oya next to Skyvern for healthcare and payer-portal automation. */
export const VS_SKYVERN: AnswerPageContent = {
  path: '/vs/skyvern',
  label: 'Oya vs Skyvern',
  title: 'Oya vs Skyvern: payer portal and browser automation compared',
  lead: 'Comparing Oya Browser and Skyvern for healthcare or portal automation? Skyvern offers a self-serve API, AGPL-3.0 self-hosting, SOC 2 Type II and cached Playwright scripts, and says HIPAA compliant without stating a BAA on the pages reviewed. Oya signs BAAs on Oya Cloud and the enterprise edition, keeps an append-only hash-chained audit trail, seals passwords and TOTP, and exports readable Playwright playbooks you own.',
  hero: {
    headline: 'Oya vs Skyvern: evaluate portal replay, audit trails and BAA scope.',
    paragraphs: [
      'For healthcare and automation teams comparing Skyvern and Oya, both automate portals with no API and replay Playwright code. Compare BAA commitments, audit-log guarantees, live takeover and how you own the resulting code.',
    ],
    action: DEMO_ACTION,
  },
  sections: [
    {
      heading: 'Start with a real payer-portal or back-office workflow.',
      body: 'Pick one recurring portal task such as a prior authorization, eligibility check or status lookup. Trace where credentials, two-factor prompts, recordings and PHI flow before comparing platforms.',
    },
    COMPARISON_REQUIREMENTS,
    {
      ...PUBLISHED_DIFFERENCES,
      table: compareTable('Oya and Skyvern, from each vendor\u2019s own pages', PORTAL_ROWS, [OYA, SKYVERN]),
      fit: {
        oya: {
          heading: 'Choose Oya when',
          points: [
            'You need a signed Business Associate Agreement (BAA) on Oya Cloud or in your own cloud under the enterprise edition.',
            'You require an append-only, hash-chained audit trail of every agent and person action, kept six years in HIPAA mode.',
            'You want standalone Playwright playbooks you own, sealed passwords and TOTP, plus a free desktop app for staff and coding agents.',
          ],
        },
        other: {
          heading: 'Choose Skyvern when',
          points: [
            'You want lower-cost self-serve cloud tiers ($29 or $149 a month) for non-BAA workflows.',
            'You want an AGPL-3.0 self-hostable automation platform with built-in TOTP, email and SMS code handling.',
            'You already have a Skyvern API workflow and do not require a publicly stated BAA or hash-chained ePHI audit trail.',
          ],
        },
      },
    },
    {
      heading: 'Check BAA scope and code ownership before production.',
      body: 'Oya does not route browsers through Skyvern. Before sending protected health information through any portal automation tool, verify the signed BAA, audit retention and whether your team keeps runnable Playwright modules if you ever change vendors.',
    },
    COMPARISON_CLOSING,
  ],
  faq: [
    {
      q: 'What is the difference between Oya Browser and Skyvern for healthcare?',
      a: `On the pages reviewed on ${CHECKED}, Skyvern states SOC 2 Type II and says HIPAA compliant without stating a BAA or an audit trail. Oya signs Business Associate Agreements (BAAs) for HIPAA mode on Oya Cloud and the enterprise edition, keeps an append-only hash-chained audit trail for six years in HIPAA mode, and exports every playbook as standalone Playwright code.`,
    },
    {
      q: 'Do both Oya and Skyvern replay tasks without calling a model every time?',
      a: 'Yes. Skyvern replays cached Playwright code and falls back to AI on failure. Oya saves each finished task as a readable Playwright playbook that replays with no model and uses an agent to repair only a step that moved.',
    },
    {
      q: 'How do Oya and Skyvern handle MFA and human takeover?',
      a: 'Skyvern states support for TOTP, email and SMS codes plus live view. Oya seals TOTP seeds so the model never sees them, and lets staff watch any session live, take over for a text-message code or review step, and hand control back.',
    },
    {
      q: 'Can I self-host Oya instead of Skyvern?',
      a: 'Yes. Skyvern self-hosts under AGPL-3.0. Oya offers a source-available community edition free up to 5 browsers, and an enterprise edition on Docker, ECS, GCP or Kubernetes that includes the HIPAA offering and a BAA.',
    },
    SELF_HOST_FAQ,
  ],
  sources: { checked: CHECKED, links: SKYVERN.sources },
};
