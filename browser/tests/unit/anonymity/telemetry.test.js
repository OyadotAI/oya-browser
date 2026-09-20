/**
 * Unit tests for anonymity/telemetry.js: the Chromium switches and the
 * telemetry host block list.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { applyTelemetryFlags, applyDomainBlocking } = require('../../../anonymity/telemetry');

/** A session whose webRequest keeps the last handler installed. */
function fakeSession() {
  const ses = { calls: [], webRequest: { onBeforeRequest: (fn) => ses.calls.push(fn) } };
  return ses;
}

/** What the installed handler answers for one URL. */
function decide(ses, url) {
  let answer;
  ses.calls.at(-1)({ url }, (value) => (answer = value));
  return answer;
}

describe('applyTelemetryFlags', () => {
  it('disables the phone-home features and background traffic', () => {
    const switches = [];
    applyTelemetryFlags({ commandLine: { appendSwitch: (...args) => switches.push(args) } });
    assert.equal(switches[0][0], 'disable-features');
    assert.ok(switches[0][1].split(',').includes('SafeBrowsing'));
    assert.deepEqual(switches[1], ['disable-background-networking']);
    assert.deepEqual(switches.at(-1), ['disable-hang-monitor']);
    assert.equal(switches.length, 12);
  });
});

describe('applyDomainBlocking', () => {
  it('clears any previous handler before installing its own', () => {
    const ses = fakeSession();
    applyDomainBlocking(ses);
    assert.equal(ses.calls[0], null);
    assert.equal(typeof ses.calls[1], 'function');
  });

  it('cancels a telemetry host and its subdomains', () => {
    const ses = fakeSession();
    applyDomainBlocking(ses);
    assert.deepEqual(decide(ses, 'https://update.googleapis.com/x'), { cancel: true });
    assert.deepEqual(decide(ses, 'https://a.clients2.google.com/'), { cancel: true });
  });

  it('lets ordinary, local and unparsable URLs through', () => {
    const ses = fakeSession();
    applyDomainBlocking(ses);
    assert.deepEqual(decide(ses, 'https://example.com/'), {});
    assert.deepEqual(decide(ses, 'https://notclients2.google.com.evil/'), {});
    assert.deepEqual(decide(ses, 'file:///tmp/a.html'), {});
    assert.deepEqual(decide(ses, 'not a url'), {});
  });
});
