// Reddit at scale: 10 Oya browsers search in parallel, driven by plain Playwright.
// Reddit blocks datacenter IPs, so run as a profile with a residential proxy pinned. See README.md.
import { writeFileSync } from 'node:fs';
import { chromium, type Page } from 'playwright-core';
import { Oya } from '@oya-ai/browser';

const QUERIES = [
  'cloud browser automation', 'browserbase alternative', 'AI browser agent', 'anti-detect browser',
  'playwright scraping at scale', 'captcha solving automation', 'browser fingerprint detection',
  'browser-use agent', 'residential proxy scraping', 'MCP browser automation',
];
const HITS = 5;      // search results kept per query
const POSTS = 3;     // of those, posts opened for body and comments
const COMMENTS = 5;  // top-level comments kept per post

const oya = new Oya(); // OYA_API_KEY, and OYA_BASE_URL for a self-hosted server

/** One search: its top hits, then the top posts with body and comments. */
async function search(page: Page, query: string) {
  await page.goto(`https://www.reddit.com/search/?q=${encodeURIComponent(query)}&type=posts&t=year`, { waitUntil: 'domcontentloaded' });
  await page.waitForSelector('a[data-testid="post-title"]', { timeout: 30_000 }).catch(async () => {
    throw new Error(`no results (${await page.title() || 'untitled page'}): ${(await page.innerText('body')).slice(0, 120)}`);
  });
  
  const hits = await page.$$eval('a[data-testid="post-title"]', (links, max) =>
    links.slice(0, max).map((a) => ({ title: a.textContent!.trim(), url: (a as HTMLAnchorElement).href })), HITS);

  const posts = [];
  for (const hit of hits.slice(0, POSTS)) {
    await page.goto(hit.url, { waitUntil: 'domcontentloaded' });
    await page.waitForSelector('shreddit-post', { timeout: 30_000 });
    await page.waitForSelector('shreddit-comment', { timeout: 10_000 }).catch(() => {}); // some posts have none
    // No named helpers in here: this runs in the page, where tsx's __name wrapper does not exist.
    posts.push({ ...hit, ...await page.evaluate((max) => {
      const post = document.querySelector('shreddit-post')!;
      return {
        subreddit: post.getAttribute('subreddit-prefixed-name'),
        score: Number(post.getAttribute('score')),
        comments: Number(post.getAttribute('comment-count')),
        created: post.getAttribute('created-timestamp'),
        body: (post.querySelector('[slot="text-body"]') as HTMLElement | null)?.innerText.trim().slice(0, 2000) ?? '',
        topComments: [...document.querySelectorAll('shreddit-comment[depth="0"]')].slice(0, max).map((c) => ({
          score: Number(c.getAttribute('score')),
          text: (c.querySelector('[slot="comment"]') as HTMLElement | null)?.innerText.trim().slice(0, 500) ?? '',
        })),
      };
    }, COMMENTS) });
  }
  return { query, hits, posts };
}

/** Playwright on an Oya browser. Media is skipped, because residential proxies bill per GB. */
async function open(cdpUrl: string | undefined) {
  if (!cdpUrl) throw new Error('No CDP URL: update the Oya server, or start the browser with OYA_REMOTE_DEBUGGING_PORT.');
  const pw = await chromium.connectOverCDP(cdpUrl);
  const context = pw.contexts()[0];
  context.setDefaultTimeout(60_000);
  await context.route(/\.(png|jpe?g|gif|webp|avif|mp4|webm|woff2?)(\?|$)/i, (route) => route.abort());
  return { pw, page: context.pages()[0] ?? await context.newPage() };
}

type Result = Awaited<ReturnType<typeof search>> | { query: string; error: string };
const failed = (query: string) => (err: unknown): Result => ({ query, error: String(err) });

/** A fresh cloud browser per query, stopped when done even if the search throws. */
async function inCloud(query: string): Promise<Result> {
  await using browser = await oya.browser.start({ persona: process.env.OYA_PERSONA || 'default', name: `reddit: ${query}` });
  const { pw, page } = await open(browser.cdpUrl);
  try { return await search(page, query); } finally { await pw.close(); }
}

/** A browser that is already running, such as your desktop app: one query at a time, left running. */
async function inRunningBrowser(id: string): Promise<Result[]> {
  const { pw, page } = await open((await oya.browser.get(id)).cdpUrl);
  const results: Result[] = [];
  for (const query of QUERIES) results.push(await search(page, query).catch(failed(query)));
  await pw.close();
  return results;
}

const started = Date.now();
const results = process.env.OYA_BROWSER_ID
  ? await inRunningBrowser(process.env.OYA_BROWSER_ID)
  : await Promise.all(QUERIES.map((query) => inCloud(query).catch(failed(query))));

const ok = results.filter((r) => !('error' in r)).length;
console.log(`${ok}/${QUERIES.length} searches ok in ${Math.round((Date.now() - started) / 1000)}s`);
for (const r of results) console.log('error' in r ? `  ✗ ${r.query}: ${r.error.split('\n')[0]}` : `  ✓ ${r.query}: ${r.posts.length} posts`);
// A run where everything failed (an empty proxy balance, say) keeps the last good results.
if (ok) writeFileSync(new URL('results.json', import.meta.url), JSON.stringify(results, null, 2));
