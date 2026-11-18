/**
 * The one line each kind of user changes to run on Oya, shared by the home page and the top of the docs so the two
 * cannot drift. Only lines checked against Oya Cloud belong here, each run live on 2026-11-18: Playwright over CDP
 * (page title read), and browser-use 1.0.6 (its daemon connected, made a second CDP call and stopped). Puppeteer and
 * Stagehand v4 are left out until they are run; Stagehand v4 needs its extension in the browser, which Oya does not
 * load yet.
 */

/** One way in: who it is for, the line they change, and what it assumes. */
export interface StackLine {
  /** A stable id, for keys and anchors. */
  id: string;
  /** Who this is for, as they would describe themselves. */
  who: string;
  /** The language of the line, shown beside it. */
  lang: 'Python' | 'TypeScript' | 'Shell';
  /** The one line. */
  line: string;
  /** What the line assumes, in a few words. */
  note: string;
}

/** The ways in, most-installed first. */
export const STACK_LINES: StackLine[] = [
  {
    id: 'browser-use',
    who: 'browser-use',
    lang: 'Python',
    line: 'agent = Agent(task, llm, browser=await browser.for_browser_use())',
    note: 'browser = await AsyncOya().browser.start(), from pip install "oya-browser[browser-use]"',
  },
  {
    id: 'playwright-python',
    who: 'Playwright',
    lang: 'Python',
    line: 'cdp = await p.chromium.connect_over_cdp(await browser.fresh_cdp_url())',
    note: 'browser = await AsyncOya().browser.start(), from pip install oya-browser',
  },
  {
    id: 'playwright-ts',
    who: 'Playwright',
    lang: 'TypeScript',
    line: 'const cdp = await chromium.connectOverCDP((await new Oya().browser.start()).cdpUrl!);',
    note: 'from npm i @oya-ai/browser',
  },
  {
    id: 'browserbase',
    who: 'Coming from Browserbase',
    lang: 'TypeScript',
    line: 'const cdp = await chromium.connectOverCDP(browser.cdpUrl!); // was session.connectUrl',
    note: "or keep Browserbase behind Oya: oya.browser.start({ provider: 'browserbase' }) with your key under Providers",
  },
  {
    id: 'mcp',
    who: 'Claude Code, Codex, Cursor',
    lang: 'Shell',
    line: 'claude mcp add --transport http oya https://oyabrowser.com/mcp/pool --header "Authorization: Bearer $OYA_API_KEY"',
    note: 'or click Connect in the Oya desktop app',
  },
];

/** The section's heading and lead, the same on the home page and in the docs. */
export const STACK_HEADING = 'Already have a stack? Change one line.';

/** What the lines have in common, said once. */
export const STACK_LEAD =
  'Your agent code stays as it is. Point it at an Oya browser and it gets playbooks that replay with no model, a tamper-evident audit trail, and a person who can take over.';
