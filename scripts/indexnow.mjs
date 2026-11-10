/**
 * Tells Bing, Yandex and the other IndexNow engines (and through Bing, ChatGPT search and Copilot)
 * that every page in the live sitemap changed, so new answer pages are crawled in hours, not weeks.
 * Run it after a site deploy:
 *
 *   node scripts/indexnow.mjs            submit every URL in https://oyabrowser.com/sitemap.xml
 *   node scripts/indexnow.mjs --dry-run  print what would be submitted
 *
 * The key is public by design: IndexNow proves ownership by fetching ui/public/<key>.txt.
 */
import { pathToFileURL } from 'node:url';

/** The key whose file ui/public/<key>.txt serves; change both together. */
export const KEY = '6133278fca7b72df2ea13fab91b68091';

/** The site whose sitemap is submitted. */
const SITE = (process.env.SITE_URL || 'https://oyabrowser.com').replace(/\/$/, '');

/** The shared IndexNow endpoint, which forwards to every participating engine. */
const ENDPOINT = 'https://api.indexnow.org/indexnow';

/** Every <loc> in a sitemap, in order. */
export function sitemapUrls(xml) {
  return [...xml.matchAll(/<loc>\s*([^<\s]+)\s*<\/loc>/g)].map((m) => m[1]);
}

/** The IndexNow request body for a site and its URLs. */
export function submission(site, urls) {
  return { host: new URL(site).host, key: KEY, keyLocation: `${site}/${KEY}.txt`, urlList: urls };
}

/** Fetches the sitemap and submits it; a non-2xx answer fails the run. */
async function main(dryRun) {
  const body = submission(SITE, sitemapUrls(await (await fetch(`${SITE}/sitemap.xml`)).text()));
  if (dryRun) return console.log(JSON.stringify(body, null, 2));
  const res = await fetch(ENDPOINT, {
    method: 'POST',
    headers: { 'content-type': 'application/json; charset=utf-8' },
    body: JSON.stringify(body),
  });
  console.log(`IndexNow: ${res.status} for ${body.urlList.length} URLs`);
  if (!res.ok) process.exit(1);
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) await main(process.argv.includes('--dry-run'));
