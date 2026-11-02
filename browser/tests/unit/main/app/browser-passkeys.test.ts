/** Metadata-only native policy gates retain exact source ownership through OS consent, cancellation and final success. */
import { it, type TestContext } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { randomUUID } from 'node:crypto';
import { BrowserPasskeys } from '../../../../src/main/app/browser-passkeys.ts';
import { fixture as passkeyFixture } from '../../support/passkey-fixture.ts';
import { PASSKEY_PROMPT_TIMEOUT_MS } from '../../../../src/main/app/constants.ts';
import type { BrowserPasskeyDetails } from '../../../../src/main/app/browser-passkey-types.ts';

/** No real account, credential, provider, Bluetooth state or biometric is involved in this native-event seam. */
function fixture(t: TestContext) {
  const f = passkeyFixture(),
    session = new EventEmitter(),
    requests = new Set<string>();
  Object.assign(f.details.frame, { url: 'https://accounts.fixture.test/signin' });
  Object.assign(f.contents, { session, mainFrame: f.details.frame, getURL: () => f.details.frame.url });
  Object.assign(f.deps, { config: { values: {} }, recorder: { recording: false } });
  f.deps.persona.session = () => session;
  const gate = new BrowserPasskeys(f.deps);
  gate.install(session as any);
  const details: BrowserPasskeyDetails = {
    requestId: randomUUID(),
    phase: 'start',
    frame: f.details.frame,
    relyingPartyId: 'fixture.test',
    origin: 'https://accounts.fixture.test',
    kind: 'get',
    presentation: 'modal',
    transports: ['hybrid'],
  };
  let cancellations = 0;
  const cancel = () => {
    cancellations++;
  };
  const end = (id = details.requestId, owner = session) =>
    owner.emit('oya-webauthn-ended', {}, { requestId: id, outcome: 'ended' });
  t.after(() => {
    for (const id of requests) end(id);
  });
  const send = (patch: Partial<BrowserPasskeyDetails> = {}, owner = session, decision?: (allow: boolean) => void) => {
    const value = { ...details, ...patch },
      answers: boolean[] = [];
    requests.add(value.requestId);
    owner.emit(
      'oya-webauthn-request',
      {},
      value,
      decision ??
        ((allow: boolean) => {
          answers.push(allow);
        }),
      cancel,
    );
    return answers;
  };
  return { ...f, session, gate, details, send, end, cancellations: () => cancellations };
}

it('admits only one exact native request and requires a separate one-use final success decision', (t) => {
  const f = fixture(t);
  assert.deepEqual(f.send(), [true]);
  assert.equal(f.boxes.length, 0);
  assert.deepEqual(f.send({ phase: 'complete' }), [true]);
  assert.deepEqual(f.send({ phase: 'complete' }), [false]);
  assert.equal(f.cancellations(), 1);
  f.end();
  assert.equal(f.contents.listenerCount('did-start-navigation'), 0);
  assert.deepEqual(f.send({ requestId: randomUUID() }), [true]);
});

it('another native attempt is denied without borrowing or canceling the already-running request', (t) => {
  const f = fixture(t);
  assert.deepEqual(f.send(), [true]);
  assert.deepEqual(f.send({ requestId: randomUUID() }), [false]);
  assert.equal(f.cancellations(), 1);
  assert.deepEqual(f.send({ phase: 'complete' }), [true]);
});

it('OS-sheet parent blur is narrowly allowed while another tracked Oya window revokes consent', (t) => {
  const f = fixture(t);
  f.send();
  f.window.isFocused = () => false;
  assert.deepEqual(f.send({ phase: 'complete' }), [true]);
  f.end();
  f.window.isFocused = () => true;
  const next = randomUUID();
  f.send({ requestId: next });
  const other = { webContents: {} };
  f.deps.windows.fromContents = (contents: unknown) => (contents === other.webContents ? f.owner : undefined);
  f.deps.electron.app.emit('browser-window-focus', {}, other);
  assert.equal(f.cancellations(), 1);
  assert.deepEqual(f.send({ requestId: next, phase: 'complete' }), [false]);
});

it('native navigation and switched-away-and-back control/profile cannot revive an allowed request', (t) => {
  const changes = [
    (f: any) => {
      f.contents.emit('did-start-navigation', { isMainFrame: true, isSameDocument: false, url: f.details.origin });
    },
    (f: any) => {
      f.deps.agentTab.humanMay = () => false;
      f.gate.sync();
      f.deps.agentTab.humanMay = () => true;
    },
    (f: any) => {
      const epoch = f.deps.persona.policyEpoch;
      f.deps.persona.policyEpoch = Symbol();
      f.gate.sync();
      f.deps.persona.policyEpoch = epoch;
    },
  ];
  for (const change of changes) {
    const f = fixture(t);
    f.send();
    change(f);
    assert.equal(f.cancellations(), 1);
    assert.deepEqual(f.send({ phase: 'complete' }), [false]);
  }
});

it('same-document and unrelated-frame activity preserves the original eligible request', (t) => {
  const f = fixture(t);
  f.send();
  f.contents.emit('did-start-navigation', { isMainFrame: true, isSameDocument: true, frame: f.details.frame });
  f.contents.emit('did-start-navigation', { isMainFrame: false, isSameDocument: false, frame: {} });
  assert.equal(f.cancellations(), 0);
  assert.deepEqual(f.send({ phase: 'complete' }), [true]);
});

it('the final phase refuses a replaced frame/session, changed origin/RP, recording and governed contexts', (t) => {
  const changes = [
    (f: any) => {
      f.contents.mainFrame = { ...f.details.frame };
    },
    (f: any) => {
      f.contents.session = {};
    },
    (f: any) => {
      f.details.frame.url = 'https://other.fixture.test';
    },
    (f: any) => {
      f.details.relyingPartyId = 'other.test';
    },
    (f: any) => {
      f.deps.recorder.recording = true;
    },
    (f: any) => {
      f.deps.governance.configuration = {};
    },
  ];
  for (const change of changes) {
    const f = fixture(t);
    f.send();
    change(f);
    assert.deepEqual(f.send({ phase: 'complete' }), [false]);
    assert.equal(f.cancellations(), 1);
  }
});

it('metadata and background admission fail closed without creating account dialogs', (t) => {
  const patches = [
    { requestId: '' },
    { requestId: 'x'.repeat(129) },
    { origin: 'https://other.test' },
    { kind: 'create' },
    { presentation: 'conditional' },
    { transports: ['invented'] },
    { frame: null },
  ];
  for (const patch of patches) {
    const f = fixture(t);
    assert.deepEqual(f.send(patch as any), [false]);
    assert.equal(f.boxes.length, 0);
  }
  const f = fixture(t);
  f.window.isFocused = () => false;
  assert.deepEqual(f.send(), [false]);
});

it('request timeout revokes native consent once and keeps the main lease occupied until native end', (t) => {
  t.mock.timers.enable({ apis: ['setTimeout', 'setInterval'] });
  const f = fixture(t);
  f.send();
  t.mock.timers.tick(PASSKEY_PROMPT_TIMEOUT_MS);
  f.gate.sync();
  assert.equal(f.cancellations(), 1);
  assert.equal(f.contents.listenerCount('did-start-navigation'), 0);
  assert.deepEqual(f.send({ phase: 'complete' }), [false]);
  assert.deepEqual(f.send({ requestId: randomUUID() }), [false]);
  f.end();
  assert.deepEqual(f.send({ requestId: randomUUID() }), [true]);
});

it('a wrong session end/complete cannot release the original request or its account-selection capability', (t) => {
  const f = fixture(t),
    other = new EventEmitter();
  f.gate.install(other as any);
  f.send();
  f.end(f.details.requestId, other);
  assert.deepEqual(f.send({ phase: 'complete' }, other), [false]);
  assert.equal(f.gate.account({ ...f.details, requestId: f.details.requestId, accounts: [] } as any), true);
  assert.equal(f.gate.account({ ...f.details, requestId: randomUUID(), accounts: [] } as any), false);
  assert.equal(f.gate.account({ ...f.details, requestId: undefined, accounts: [] } as any), false);
  assert.deepEqual(f.send({ phase: 'complete' }), [true]);
  assert.equal(f.gate.account({ ...f.details, requestId: f.details.requestId, accounts: [] } as any), false);
});

it('an exception after a recorded allow revokes its native lifetime before dispatch can close', (t) => {
  const f = fixture(t),
    answers: boolean[] = [];
  f.send({}, f.session, (allow) => {
    answers.push(allow);
    throw Error('fixture-sensitive-detail');
  });
  assert.deepEqual(answers, [true]);
  assert.equal(f.cancellations(), 1);
  assert.deepEqual(f.send({ phase: 'complete' }), [false]);
});

it('reused sessions install exactly one gate and terminal disposal releases owned observers', (t) => {
  const f = fixture(t);
  f.gate.install(f.session as any);
  assert.equal(f.session.listenerCount('oya-webauthn-request'), 1);
  assert.equal(f.session.listenerCount('oya-webauthn-ended'), 1);
  f.send();
  f.end();
  assert.equal(f.cancellations(), 0);
  assert.equal(f.contents.listenerCount('did-start-navigation'), 0);
  assert.equal(f.window.listenerCount('closed'), 0);
});
