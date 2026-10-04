/**
 * Unit tests for src/main/identity/governance.ts: what a governed browser lets
 * out (policies, schemes, human-only hosts by control mode), what an ungoverned
 * one does, and how OYA_GOVERNANCE is read. The session is faked.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { Governance, readGovernance } from '../../../../src/main/identity/governance.ts';

/** A governed browser allowed onto example.com, with bank.example.com held for a person. */
const governed = () =>
  new Governance({ policies: [{ allowedHosts: ['*.example.com', 'example.com'], humanHosts: ['bank.example.com'] }] });

/** A session that records the hooks governance installs. */
function fakeSession(): any {
  return {
    webRequest: {
      onBeforeRequest(fn: any) {
        this.before = fn;
      },
    },
    setPermissionRequestHandler(fn: any) {
      this.request = fn;
    },
    setPermissionCheckHandler(fn: any) {
      this.check = fn;
    },
  };
}

describe('Governance', () => {
  it('allows everything and installs nothing when the browser is not governed', () => {
    const governance = new Governance(null);
    const ses = fakeSession();
    governance.install(ses);
    assert.equal(governance.allowed('https://anything.test/'), true);
    assert.equal(ses.webRequest.before, undefined);
    assert.equal(ses.request, undefined);
  });

  it('allows only the policy hosts, over http(s) and ws(s)', () => {
    const governance = governed();
    assert.equal(governance.allowed('https://www.example.com/a'), true);
    assert.equal(governance.allowed('wss://example.com/socket'), true);
    assert.equal(governance.allowed('https://other.test/'), false);
    assert.equal(governance.allowed('file:///etc/passwd'), false);
    assert.equal(governance.allowed('not a url'), false);
  });

  it('always allows about:blank', () => {
    assert.equal(governed().allowed('about:blank'), true);
  });

  it('opens human-only hosts to a person alone', () => {
    const governance = governed();
    assert.equal(governance.allowed('https://bank.example.com/'), false);
    governance.setMode('human');
    assert.equal(governance.allowed('https://bank.example.com/'), true);
    governance.setMode('paused');
    assert.equal(governance.allowed('https://bank.example.com/'), false);
  });

  it('requires every policy to allow a request', () => {
    const governance = new Governance({
      policies: [{ allowedHosts: ['*.example.com'] }, { allowedHosts: ['a.example.com'] }],
    });
    assert.equal(governance.allowed('https://a.example.com/'), true);
    assert.equal(governance.allowed('https://b.example.com/'), false);
  });

  it('cancels refused requests and denies every permission once installed', () => {
    const ses = fakeSession();
    governed().install(ses);
    const answers: any[] = [];
    ses.webRequest.before({ url: 'https://other.test/' }, (a: any) => answers.push(a));
    ses.webRequest.before({ url: 'https://example.com/' }, (a: any) => answers.push(a));
    ses.request(null, 'camera', (granted: boolean) => answers.push(granted));
    assert.deepEqual(answers, [{ cancel: true }, { cancel: false }, false]);
    assert.equal(ses.check(), false);
  });
});

describe('readGovernance', () => {
  it('reads OYA_GOVERNANCE, and nothing as ungoverned', () => {
    assert.deepEqual(readGovernance('{"policies":[]}'), { policies: [] });
    assert.equal(readGovernance(undefined), null);
    assert.equal(readGovernance(''), null);
  });

  it('refuses malformed JSON rather than starting open', () => {
    assert.throws(() => readGovernance('{'));
  });
});
