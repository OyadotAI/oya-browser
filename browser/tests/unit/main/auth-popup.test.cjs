/**
 * Unit tests for main/auth-popup.cjs: only real sign-in providers (matched on
 * the hostname) and explicit popups stay windows.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { isAuthPopup } = require('../../../main/auth-popup.cjs');

describe('isAuthPopup', () => {
  it('keeps a provider and its subdomains as a window', () => {
    assert.equal(isAuthPopup('https://accounts.google.com/o/oauth2', ''), true);
    assert.equal(isAuthPopup('https://api.twitter.com/oauth', ''), true);
  });

  it('matches on the hostname, not a substring of the URL', () => {
    assert.equal(isAuthPopup('https://box.com/x.com', ''), false);
    assert.equal(isAuthPopup('https://evil.test/?next=accounts.google.com', ''), false);
  });

  it('allows GitHub only on its OAuth path', () => {
    assert.equal(isAuthPopup('https://github.com/login/oauth/authorize', ''), true);
    assert.equal(isAuthPopup('https://github.com/settings', ''), false);
  });

  it('keeps any window.open that asked for popup features', () => {
    assert.equal(isAuthPopup('https://example.com', 'popup,width=500'), true);
  });

  it('turns an unparseable URL into a tab', () => {
    assert.equal(isAuthPopup('not a url', ''), false);
    assert.equal(isAuthPopup(undefined, undefined), false);
  });
});
