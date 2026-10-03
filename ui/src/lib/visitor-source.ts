/**
 * Where a visitor came from: an ad campaign's UTM tags, an ad click id, the
 * site that linked here, or direct. Kept in a first-party cookie (oya_src) so
 * the server can say where each desktop download came from; the download link
 * is a plain file request that runs no script. Nothing in it names a person.
 *
 * A visit from an ad or a tagged link always replaces what was kept, since it
 * says the most; an untagged visit only fills an empty cookie.
 */
import { SECONDS_PER_DAY, SOURCE_MAX_LENGTH, SOURCE_MEMORY_DAYS } from './constants';

/** The cookie the server reads the source from (server/src/modules/telemetry/downloads.ts). */
const COOKIE = 'oya_src';

/** The click id each ad network adds to its links → the source it means. */
const CLICK_IDS: Record<string, string> = {
  fbclid: 'facebook ad',
  gclid: 'google ad',
  gbraid: 'google ad',
  wbraid: 'google ad',
  li_fat_id: 'linkedin ad',
  rdt_cid: 'reddit ad',
  twclid: 'x ad',
  msclkid: 'bing ad',
};

/** `utm_source / utm_campaign`, or '' when the link was not tagged. */
function utm(query: URLSearchParams) {
  return [query.get('utm_source'), query.get('utm_campaign')].filter(Boolean).join(' / ');
}

/** The ad network a click id names, or ''. */
const adClick = (query: URLSearchParams) => Object.keys(CLICK_IDS).find((id) => query.has(id)) ?? '';

/** The host that linked here, or '' when none did or it was this site. */
function referrerHost(referrer: string, own: string) {
  try {
    const host = new URL(referrer).host;
    return host === own ? '' : host.replace(/^www\./, '');
  } catch {
    return '';
  }
}

/** Lower case, only the characters a campaign name needs, and short. */
const clean = (source: string) =>
  source
    .toLowerCase()
    .replace(/[^\w.\-/ ]/g, '')
    .slice(0, SOURCE_MAX_LENGTH);

/** Where one visit came from. */
type Visit = {
  /** The source, as the server prints it. */
  source: string;
  /** Whether an ad or a tagged link brought it, which outranks a source already kept. */
  tagged: boolean;
};

/** Where this visit came from. */
export function visitSource(url: URL, referrer: string): Visit {
  const tagged = utm(url.searchParams) || CLICK_IDS[adClick(url.searchParams)] || '';
  if (tagged) return { source: clean(tagged), tagged: true };
  return { source: clean(referrerHost(referrer, url.host)) || 'direct', tagged: false };
}

/** Whether a source is already kept. */
const kept = () => document.cookie.split(';').some((c) => c.trim().startsWith(`${COOKIE}=`));

/** Keeps where this visit came from, unless an earlier one is kept and this one is untagged. */
export function rememberSource() {
  const { source, tagged } = visitSource(new URL(window.location.href), document.referrer);
  if (!tagged && kept()) return;
  const maxAge = SOURCE_MEMORY_DAYS * SECONDS_PER_DAY;
  document.cookie = `${COOKIE}=${encodeURIComponent(source)}; Max-Age=${maxAge}; Path=/; SameSite=Lax`;
}
