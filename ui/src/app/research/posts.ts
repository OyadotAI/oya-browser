/**
 * The research posts: what we measured, how, and the raw table, copied from the dated write-ups in
 * the repository's docs/ folder. Every cell here must appear in its source document (a unit test reads
 * both and fails on drift), so the site never quotes a number the write-up does not.
 */

/** One measured table: its caption, its column headings and its rows of cells. */
export interface ResearchTable {
  /** What the table shows and where it comes from, read out as the table's caption. */
  caption: string;
  /** The column headings, left to right. */
  columns: string[];
  /** Each row's cells, in column order. */
  rows: string[][];
}

/** A research post: its address, when the run happened, the method, the table and how to read it. */
export interface ResearchPost {
  /** Its path under /research, for the link, the canonical address and the sitemap. */
  path: string;
  /** The post's title and heading. */
  title: string;
  /** One sentence for the index and the description: what was measured and what came out. */
  summary: string;
  /** The day the run was made, as written in the source document. */
  date: string;
  /** What the run was made on, shown beside the date. */
  setup: string;
  /** How the run was made, so a reader can judge and repeat it. */
  method: string;
  /** The raw numbers. */
  table: ResearchTable;
  /** The caveats that come with the numbers. */
  notes: string[];
  /** The document in the repository the post is copied from. */
  source: string;
}

/** The replay benchmark: ten sites recorded once, replayed with no model and healing off. */
export const REPLAY_POST: ResearchPost = {
  path: '/research/replay',
  title: 'How well a recorded run replays',
  summary:
    'Ten public sites, recorded once by the agent and replayed with no model in the loop: nine of ten replayed every recorded step.',
  date: '2026-09-20',
  setup: 'A self-hosted stack on one Mac, Chrome 153',
  method:
    'Ten public sites, recorded once by the agent and then replayed with no model in the loop and auto-healing switched off, so a step that no longer fits fails loudly instead of being repaired behind the scenes. Healing off is the point: with it on, the agent papers over a gap and the run still succeeds, which tells you nothing about the recording.',
  table: {
    caption: 'Record and replay time per site, from docs/replay-benchmark.md',
    columns: ['Site', 'What the flow did', 'Steps', 'Record', 'Replay', 'Result'],
    rows: [
      ['wikipedia', 'follow two article links', '3', '11.8s', '10.6s', 'every step'],
      ['hackernews', "open a story's comments, return via the logo", '3', '11.7s', '8.5s', 'every step'],
      ['github', 'switch between Issues and Pull requests', '4', '29.0s', '17.5s', 'every step'],
      ['mdn', 'follow two interface links', '7', '43.1s', '23.3s', 'every step'],
      ['stackoverflow', 'open a question, then its first tag', '16', '264.6s', '111.8s', 'every step'],
      ['linkedin', 'page through search results', '8', '20.7s', '25.0s', 'every step'],
      ['amazon', 'paginate a search twice', '8', '118.3s', '116.0s', 'every step'],
      ['airbnb', 'note a listing, then the next page', '5', '20.9s', '15.6s', 'every step'],
      ['reddit', 'read a post, open its subreddit', '2', '58.5s', '36.8s', 'every step'],
      ['ebay', 'paginate a search twice', '5', '139.0s', '61.2s', 'stopped at step 3'],
    ],
  },
  notes: [
    "eBay's own pagination link redirected the click to a registration wall. The replay noticed that the element it reached was not the element it recorded and stopped. That is the intended behaviour.",
    'Replay is paced on purpose, 350–1200ms between steps and longer after a page changes. It is not "instant", and it declines to be: a faster replay on Amazon drew a challenge that a slower one did not.',
    'A step count is not a difficulty score. Two steps on reddit took longer to record than eight on LinkedIn.',
    'This is ten sites on one machine on one day. It is a measurement, not a guarantee, and the numbers move with the sites.',
  ],
  source: 'docs/replay-benchmark.md',
};

/** The stealth measurement: the same headless Chrome, bare and with a persona, against public detectors. */
export const STEALTH_POST: ResearchPost = {
  path: '/research/stealth',
  title: 'Stealth, measured',
  summary:
    'The same headless Chrome, bare and then with an Oya persona applied: CreepJS headless score 100% to 0%, Bot.Sannysoft 27 to 31 of 31.',
  date: '2026-09-20',
  setup: 'Headless Chrome 153.0.8010.52 on macOS',
  method:
    'The same headless Chrome, launched twice: once bare, once with a persona applied exactly as production does. Both then face the public detectors. This measures headless Chrome with a persona applied, not the desktop app.',
  table: {
    caption: 'Detector results, bare and with a persona, from docs/stealth.md',
    columns: ['Detector', 'Bare headless Chrome', 'With an Oya persona'],
    rows: [
      ['CreepJS headless score (lower is better)', '100%', '0%'],
      ['CreepJS stealth-tampering score (lower is better)', '0%', '0%'],
      ['Bot.Sannysoft', '27 / 31 pass', '31 / 31 pass'],
      ['Oya probe suite (29 probes, weighted, 64 points)', '55 / 64', '64 / 64'],
      ['CreepJS like-headless score (lower is better)', '31%', '25%'],
    ],
  },
  notes: [
    '"Zero detection" is neither measurable nor achievable: the published leader sits near 77% bypass. This produces a number instead.',
    'Of the remaining like-headless signals, two are headless-only rendering defaults; the other three are Android-only APIs real desktop Chrome lacks as well.',
    'A residential proxy has no place on a regulated path. It puts a third party in the middle of a request that may carry patient data, and no residential-proxy vendor will sign a BAA.',
  ],
  source: 'docs/stealth.md',
};

/** Every post, newest first; the index, the sitemap and the tests all read this list. */
export const RESEARCH_POSTS = [REPLAY_POST, STEALTH_POST];
