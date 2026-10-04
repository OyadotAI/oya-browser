/**
 * Unit tests for src/anonymity/telemetry.ts: the Chromium switches and the
 * telemetry host block list.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { applyTelemetryFlags, applyDomainBlocking } from '../../../src/anonymity/telemetry.ts';

/** A session whose webRequest keeps the last handler installed. */
function fakeSession(): any {
  const ses: any = { calls: [], webRequest: { onBeforeRequest: (fn: unknown) => ses.calls.push(fn) } };
  return ses;
}

/** What the installed handler answers for one URL. */
function decide(ses: any, url: string) {
  let answer;
  ses.calls.at(-1)({ url }, (value: unknown) => (answer = value));
  return answer;
}

describe('applyTelemetryFlags', () => {
  it('disables the phone-home features and background traffic', () => {
    const switches: any[] = [];
    applyTelemetryFlags({ commandLine: { appendSwitch: (...args: unknown[]) => switches.push(args) } } as any);
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
