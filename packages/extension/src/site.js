/**
 * The site a tab is on and the one host permission that covers its logins.
 * A login cookie often lives on the parent domain (`.google.com` for
 * mail.google.com), and the browser hides a cookie unless the extension may
 * read that cookie's own domain, so the grant names the whole site.
 */
import { COUNTRY_CODE_LENGTH, COUNTRY_SECOND_LEVELS, COUNTRY_SITE_LABELS, PLAIN_SITE_LABELS } from './constants.js';

/** The schemes a person signs in to sites over. Every other page has no logins to send. */
const WEB_SCHEMES = new Set(['http:', 'https:']);

/** The labels of the site's registrable domain: example.com, or example.co.uk under a country suffix. */
// ponytail: a short list stands in for the Public Suffix List; ship the list if a store reviewer or user hits a gap.
function siteLabels(labels) {
  const [second, tld] = labels.slice(-PLAIN_SITE_LABELS);
  const country = tld.length === COUNTRY_CODE_LENGTH && COUNTRY_SECOND_LEVELS.has(second);
  return labels.slice(-(country ? COUNTRY_SITE_LABELS : PLAIN_SITE_LABELS));
}

/** True when the host is an IP address or a single label, which has no parent domain to widen to. */
function bareHost(host) {
  return !host.includes('.') || /^[\d.]+$/.test(host) || host.startsWith('[');
}

/** The address parsed, or null when it is not one. (URL.canParse is missing before Chrome 120.) */
function parse(url) {
  try {
    return new URL(url);
  } catch {
    return null;
  }
}

/**
 * The tab's site: its host, the domain its logins can live on, and the match
 * pattern to ask for. Null for pages that are not websites (chrome://, about:,
 * file:), where the popup has nothing to send.
 */
export function siteOf(url) {
  const parsed = parse(url);
  if (!parsed || !WEB_SCHEMES.has(parsed.protocol) || !parsed.hostname) return null;
  // A trailing dot (example.com.) is the same host; kept, it would make the grant `*.com.`.
  const host = parsed.hostname.replace(/\.+$/, '');
  if (!host) return null;
  const domain = bareHost(host) ? host : siteLabels(host.split('.')).join('.');
  const pattern = bareHost(host) ? `*://${host}/*` : `*://*.${domain}/*`;
  return { url: parsed.href, host, domain, pattern };
}
