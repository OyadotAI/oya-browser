/**
 * Unit tests for anonymity/apply.js: page emulation, the injection, and
 * coverage of every child target, over a fake CDP transport.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { createPersonaApplier, emulationFor } = require('../../../anonymity/apply');
const { generateProfile } = require('../../../anonymity/fingerprint');
const { flush } = require('../support/fakes.cjs');

const profile = generateProfile({ seed: 'unit-apply' });

/** A transport that records every command and lets a test fire CDP events or fail methods. */
function transport(failing = []) {
  const sent = [];
  const listeners = {};
  const send = async (method, params, sessionId) => {
    sent.push({ method, params, sessionId });
    if (failing.includes(method)) throw new Error(`${method} failed`);
    return {};
  };
  const on = (event, fn) => (listeners[event] = fn);
  return {
    sent,
    send,
    on,
    fire: (event, params) => listeners[event](params),
    methods: () => sent.map((c) => c.method),
  };
}

describe('emulationFor', () => {
  it('turns off the automation flag and sets the persona core count first', () => {
    const [automation, cores] = emulationFor(profile);
    assert.deepEqual(automation, ['Emulation.setAutomationOverride', { enabled: false }]);
    assert.equal(cores[1].hardwareConcurrency, profile.navigator.hardwareConcurrency);
  });

  it('overrides the screen only, leaving viewport and pixel ratio alone', () => {
    const [, params] = emulationFor(profile).at(-1);
    assert.deepEqual(params, {
      width: 0,
      height: 0,
      deviceScaleFactor: 0,
      mobile: false,
      screenWidth: profile.screen.width,
      screenHeight: profile.screen.height,
    });
  });

  it('skips the screen when the host window owns it', () => {
    const methods = emulationFor(profile, { screen: false }).map(([m]) => m);
    assert.ok(!methods.includes('Emulation.setDeviceMetricsOverride'));
  });

  it('skips timezone and locale a persona does not set', () => {
    const methods = emulationFor({ ...profile, timezone: null, locale: null }).map(([m]) => m);
    assert.ok(!methods.includes('Emulation.setTimezoneOverride'));
    assert.ok(!methods.includes('Emulation.setLocaleOverride'));
  });
});

describe('createPersonaApplier', () => {
  it('applies a page in order: user agent, emulation, injection, child coverage', async () => {
    const t = transport();
    const applier = createPersonaApplier({ ...t, profile, userAgent: { userAgent: 'UA' } });
    await applier.page('s1');
    const methods = t.methods();
    assert.equal(methods[0], 'Emulation.setUserAgentOverride');
    assert.deepEqual(methods.slice(-2), ['Page.addScriptToEvaluateOnNewDocument', 'Target.setAutoAttach']);
    assert.deepEqual(t.sent.at(-1).params.filter, [{ type: 'worker' }, { type: 'iframe' }, { exclude: true }]);
    assert.ok(t.sent.every((c) => c.sessionId === 's1'));
  });

  it('reports a failed injection but carries on to child coverage', async () => {
    const t = transport(['Page.addScriptToEvaluateOnNewDocument']);
    const errors = [];
    await createPersonaApplier({ ...t, profile, onError: (what) => errors.push(what) }).page('s');
    assert.deepEqual(errors, ['injection']);
    assert.equal(t.methods().at(-1), 'Target.setAutoAttach');
  });

  it('ignores emulation a Chrome lacks without reporting it', async () => {
    const t = transport(['Emulation.setTimezoneOverride']);
    const errors = [];
    await createPersonaApplier({ ...t, profile, onError: (what) => errors.push(what) }).page('s');
    assert.deepEqual(errors, []);
  });

  it('covers a worker with the persona, then lets it run', async () => {
    const t = transport();
    createPersonaApplier({ ...t, profile });
    t.fire('Target.attachedToTarget', { sessionId: 'w', targetInfo: { type: 'worker' }, waitingForDebugger: true });
    await flush();
    assert.deepEqual(t.methods(), ['Runtime.evaluate', 'Runtime.runIfWaitingForDebugger']);
  });

  it('covers a cross-site iframe as a page', async () => {
    const t = transport();
    createPersonaApplier({ ...t, profile });
    t.fire('Target.attachedToTarget', { sessionId: 'f', targetInfo: { type: 'iframe' }, waitingForDebugger: false });
    await flush();
    assert.ok(t.methods().includes('Page.addScriptToEvaluateOnNewDocument'));
    assert.ok(!t.methods().includes('Runtime.runIfWaitingForDebugger'));
  });

  it('covers a session once until it detaches', async () => {
    const t = transport();
    createPersonaApplier({ ...t, profile });
    const attach = () => t.fire('Target.attachedToTarget', { sessionId: 'w', targetInfo: { type: 'shared_worker' } });
    attach();
    attach();
    await flush();
    assert.equal(t.methods().filter((m) => m === 'Runtime.evaluate').length, 1);
    t.fire('Target.detachedFromTarget', { sessionId: 'w' });
    attach();
    await flush();
    assert.equal(t.methods().filter((m) => m === 'Runtime.evaluate').length, 2);
  });

  it('resumes a paused target even when its setup fails', async () => {
    const t = transport(['Runtime.evaluate']);
    const errors = [];
    createPersonaApplier({ ...t, profile, onError: (what) => errors.push(what) });
    t.fire('Target.attachedToTarget', { sessionId: 'w', targetInfo: { type: 'worker' }, waitingForDebugger: true });
    await flush();
    assert.deepEqual(errors, ['worker injection']);
    assert.equal(t.methods().at(-1), 'Runtime.runIfWaitingForDebugger');
  });

  it('only resumes a target type it does not cover', async () => {
    const t = transport();
    createPersonaApplier({ ...t, profile });
    t.fire('Target.attachedToTarget', { sessionId: 'x', targetInfo: { type: 'page' }, waitingForDebugger: true });
    await flush();
    assert.deepEqual(t.methods(), ['Runtime.runIfWaitingForDebugger']);
  });

  it('asks a browser-level connection for service and shared workers', async () => {
    const t = transport(['Target.setAutoAttach']);
    const errors = [];
    await createPersonaApplier({ ...t, profile, onError: (what) => errors.push(what) }).browser();
    assert.deepEqual(t.sent[0].params.filter, [
      { type: 'service_worker' },
      { type: 'shared_worker' },
      { exclude: true },
    ]);
    assert.equal(t.sent[0].sessionId, undefined);
    assert.deepEqual(errors, ['service worker coverage']);
  });
});
