/**
 * Sign-in popups that must stay real windows: the provider's page talks back to
 * its opener, which a tab would break. Matched on the hostname, a substring
 * match let any URL that merely mentioned one of these (box.com holds "x.com")
 * open a window.
 */
const AUTH_HOSTS = [
  'accounts.google.com',
  'login.microsoftonline.com',
  'appleid.apple.com',
  'x.com',
  'twitter.com',
  'arkoselabs.com',
];

/** Targets that name no window of their own, so nothing can be posted into them later. */
const RESERVED_TARGETS = new Set(['', '_blank', '_self', '_parent', '_top']);

/**
 * A window the page gave a name to is a window the page means to use again: it
 * keeps the reference, or posts a form into it by that name, which is how a
 * payer hands a case to a delegated vendor. Answering `window.open` with a tab
 * gives the page `null`, its own script then fails on it, and the portal shows
 * its error rather than ours.
 */
function opensNamedWindow(frameName) {
  return !RESERVED_TARGETS.has(String(frameName ?? '').trim());
}

/** Whether `url` (opened with window `features`) must stay a real popup window rather than become a tab. */
function isAuthPopup(url, features) {
  if (features && features.includes('popup')) return true;
  const target = URL.parse(url);
  if (!target) return false;
  const host = target.hostname;
  return (
    AUTH_HOSTS.some((d) => host === d || host.endsWith('.' + d)) ||
    (host === 'github.com' && target.pathname.startsWith('/login/oauth'))
  );
}

module.exports = { isAuthPopup, opensNamedWindow };
