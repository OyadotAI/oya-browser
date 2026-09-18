/**
 * Sign-in popups that must stay real windows: the provider's page talks back to
 * its opener, which a tab would break. Matched on the hostname — a substring
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

module.exports = { isAuthPopup };
