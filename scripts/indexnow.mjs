/**
 * Notifies participating search engines of the URLs in the live sitemap.
 * Notification does not guarantee crawling, indexing or ranking.
 * Run it after a site deploy:
 *
 *   node scripts/indexnow.mjs            submit every URL in https://oyabrowser.com/sitemap.xml
 *   node scripts/indexnow.mjs --dry-run  print what would be submitted
 *
 * The key is public by design: IndexNow proves ownership by fetching ui/public/<key>.txt.
 */
import { pathToFileURL } from 'node:url';
import {
  INDEXNOW_FAILURE_EXIT_CODE,
  INDEXNOW_JSON_INDENT,
  INDEXNOW_REQUEST_TIMEOUT_MS,
  MILLISECONDS_PER_SECOND,
} from './constants.mjs';

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

/** Marks actionable validation failures so transport handling preserves their exact message. */
class IndexNowError extends Error {}

/** Releases bodies whose contents must never be printed or retained by the notification. */
async function discardBody(response) {
  await response.body?.cancel().catch(() => {});
}

/** Reads only a successful sitemap, keeping HTTP failures distinct from network failures. */
async function readSitemap(response) {
  if (response.ok) return response.text();
  await discardBody(response);
  throw new IndexNowError(`IndexNow: sitemap fetch failed (HTTP ${response.status}); no URLs submitted.`);
}

/** Records a submission response without trusting or logging the remote response body. */
async function submissionStatus(response, count) {
  await discardBody(response);
  if (!response.ok) throw new IndexNowError(`IndexNow: submission failed (HTTP ${response.status}) for ${count} URLs.`);
  return response.status;
}

/** Gives each request and its body one abort deadline, always releasing its timer. */
async function request(url, init, stage, consume, fetchImpl, deadlineMs) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), deadlineMs);
  try {
    const result = await consume(await fetchImpl(url, { ...init, signal: controller.signal }));
    controller.signal.throwIfAborted();
    return result;
  } catch (error) {
    throw requestError(error, stage, controller.signal.aborted, deadlineMs);
  } finally {
    clearTimeout(timer);
  }
}

/** Separates deadline expiry from an HTTP rejection or transport disconnection. */
function requestError(error, stage, timedOut, deadlineMs) {
  if (timedOut)
    return new IndexNowError(`IndexNow: ${stage} timed out after ${deadlineMs / MILLISECONDS_PER_SECOND}s.`);
  if (error instanceof IndexNowError) return error;
  return new IndexNowError(`IndexNow: ${stage} failed: ${error?.message ?? String(error)}.`);
}

/** Serializes the existing public payload once for the shared IndexNow endpoint. */
function postOptions(payload) {
  return {
    method: 'POST',
    headers: { 'content-type': 'application/json; charset=utf-8' },
    body: JSON.stringify(payload),
  };
}

/** Fetches a nonempty sitemap and optionally notifies IndexNow through an injectable network seam. */
export async function runIndexNow({
  site = SITE,
  dryRun = false,
  fetchImpl = fetch,
  deadlineMs = INDEXNOW_REQUEST_TIMEOUT_MS,
} = {}) {
  const xml = await request(`${site}/sitemap.xml`, {}, 'sitemap fetch', readSitemap, fetchImpl, deadlineMs);
  const payload = submission(site, sitemapUrls(xml));
  if (!payload.urlList.length) throw new IndexNowError('IndexNow: sitemap contains no URLs; no submission sent.');
  if (dryRun) return { payload };
  const consume = (response) => submissionStatus(response, payload.urlList.length);
  const status = await request(ENDPOINT, postOptions(payload), 'submission', consume, fetchImpl, deadlineMs);
  return { payload, status };
}

/** Keeps console output and process status at the CLI boundary so tests never terminate their runner. */
async function main(dryRun) {
  try {
    const { payload, status } = await runIndexNow({ dryRun });
    console.log(
      dryRun
        ? JSON.stringify(payload, null, INDEXNOW_JSON_INDENT)
        : `IndexNow: ${status} for ${payload.urlList.length} URLs (notification received)`,
    );
  } catch (error) {
    console.error(error.message);
    process.exitCode = INDEXNOW_FAILURE_EXIT_CODE;
  }
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) await main(process.argv.includes('--dry-run'));
