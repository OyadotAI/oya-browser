/** Native policy installation fails closed without mutating on invalid input or using a protocol fallback. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { NativeSessionPolicies } from '../../../../src/main/native-policy/index.ts';

/** Fresh caller-owned input lets each rule verify copying and mutation safety independently. */
function policy() {
  return { timeZone: 'UTC', locale: 'en-US', hardwareConcurrency: 8, languages: ['en-US', 'en'] };
}
/** The native seam records ordering and enforces receiver identity, with injected failure points. */
function fixture(fail = '') {
  const calls: string[] = [];
  const state = {
    version: 1,
    rendererStarted: false,
    timeZone: '',
    locale: '',
    hardwareConcurrency: 0,
    acceptLanguages: '',
  };
  const session = {
    _getOyaSessionPolicy() {
      assert.equal(this, session);
      return { ...state };
    },
    _setOyaTimeZone(value: string) {
      step(this, 'zone');
      state.timeZone = value;
    },
    _setOyaHardwareConcurrency(value: number) {
      step(this, 'cores');
      state.hardwareConcurrency = value;
    },
    _setOyaLocale(value: string) {
      step(this, 'locale');
      state.locale = value;
      state.acceptLanguages = value.includes('-') ? value + ',' + value.split('-')[0] : value;
    },
  };
  /** Fail at one explicit setter while preserving a trace of attempted native mutations. */
  function step(receiver: unknown, name: string) {
    assert.equal(receiver, session);
    calls.push(name);
    if (fail === name) throw Error('engine failure');
  }
  return { session, state, calls };
}

test('installs the complete subset once and retains each native method receiver', () => {
  const owner = new NativeSessionPolicies(),
    { session, calls } = fixture();
  assert.throws(() => owner.assertConfigured(session), /not been configured/);
  const result = owner.configure(session, policy());
  assert.deepEqual(calls, ['zone', 'cores', 'locale']);
  assert.equal(owner.assertConfigured(session), result);
  assert.equal(owner.configure(session, policy()), result);
  assert.deepEqual(calls, ['zone', 'cores', 'locale']);
});

test('canonical aliases reuse one immutable snapshot without retaining caller objects', () => {
  const owner = new NativeSessionPolicies(),
    { session } = fixture(),
    input = policy();
  input.timeZone = 'US/Eastern';
  input.locale = 'EN-us';
  input.languages = ['EN-us', 'EN'];
  const result = owner.configure(session, input);
  input.locale = 'fr-FR';
  input.languages.push('fr');
  assert.equal(result.timeZone, 'America/New_York');
  assert.equal(result.locale, 'en-US');
  assert.deepEqual(result.languages, ['en-US', 'en']);
  assert.ok(Object.isFrozen(result) && Object.isFrozen(result.languages));
  assert.equal(owner.configure(session, { ...policy(), timeZone: 'America/New_York' }), result);
});

for (const count of [1, 256])
  test('accepts supported processor boundary ' + count, () => {
    const { session } = fixture();
    assert.equal(
      new NativeSessionPolicies().configure(session, { ...policy(), hardwareConcurrency: count }).hardwareConcurrency,
      count,
    );
  });

test('primary-only locales require no duplicate fallback', () => {
  const { session } = fixture();
  const result = new NativeSessionPolicies().configure(session, { ...policy(), locale: 'de', languages: ['de'] });
  assert.deepEqual(result.languages, ['de']);
});

const invalid = [
  null,
  [],
  {},
  { ...policy(), userAgent: 'unsupported' },
  { ...policy(), [Symbol('hidden')]: true },
  ...['', 'und', 'x-private', 'en_US', 'en-US-!', 'en\0US', 'é', 'x'.repeat(129), 1].map((locale) => ({
    ...policy(),
    locale,
  })),
  ...['', '+01:00', 'Not/AZone', 'UTC\0hidden', 'x'.repeat(129), null].map((timeZone) => ({ ...policy(), timeZone })),
  ...[0, -1, 1.5, 257, NaN, Infinity, '8', true, null].map((hardwareConcurrency) => ({
    ...policy(),
    hardwareConcurrency,
  })),
  ...[undefined, [], ['en-US'], ['en', 'en-US'], ['en-US', 'fr'], ['en-US', 'en', 'fr'], new Array(2), 'en-US,en'].map(
    (languages) => ({ ...policy(), languages }),
  ),
];
for (const [i, input] of invalid.entries())
  test('invalid policy ' + i + ' never reaches a native setter', () => {
    const owner = new NativeSessionPolicies(),
      { session, calls } = fixture();
    assert.throws(() => owner.configure(session, input));
    assert.deepEqual(calls, []);
    owner.configure(session, policy());
    assert.equal(owner.assertConfigured(session).locale, 'en-US');
  });

for (const method of ['_getOyaSessionPolicy', '_setOyaTimeZone', '_setOyaHardwareConcurrency', '_setOyaLocale'])
  test('missing ' + method + ' rejects the engine before any setter', () => {
    const { session, calls } = fixture();
    delete (session as any)[method];
    assert.throws(() => new NativeSessionPolicies().configure(session, policy()), /Unsupported native/);
    assert.deepEqual(calls, []);
  });

for (const readback of [null, {}, { version: 0 }, { version: 2 }])
  test('incomplete native readback ' + JSON.stringify(readback) + ' cannot enable policy', () => {
    const { session, calls } = fixture();
    (session as any)._getOyaSessionPolicy = () => readback;
    assert.throws(() => new NativeSessionPolicies().configure(session, policy()), /Unsupported native policy readback/);
    assert.deepEqual(calls, []);
  });

test('an already-started renderer fails before any immutable setter is called', () => {
  const { session, state, calls } = fixture();
  state.rendererStarted = true;
  assert.throws(() => new NativeSessionPolicies().configure(session, policy()), /before any renderer/);
  assert.deepEqual(calls, []);
});

for (const stage of ['zone', 'cores', 'locale'])
  test('failure during ' + stage + ' permanently retires that session for its owner', () => {
    const owner = new NativeSessionPolicies(),
      { session, calls } = fixture(stage);
    assert.throws(() => owner.configure(session, policy()), /installation failed/);
    const attempted = [...calls];
    assert.throws(() => owner.assertConfigured(session), /retire this session/);
    assert.throws(() => owner.configure(session, policy()), /retire this session/);
    assert.deepEqual(calls, attempted);
    const fresh = fixture();
    owner.configure(fresh.session, policy());
    assert.equal(owner.assertConfigured(fresh.session).locale, 'en-US');
  });

test('a changed policy cannot repurpose a session or invalidate its original installed binding', () => {
  const owner = new NativeSessionPolicies(),
    { session, calls } = fixture();
  const installed = owner.configure(session, policy());
  assert.throws(() => owner.configure(session, { ...policy(), hardwareConcurrency: 4 }), /cannot change/);
  assert.equal(owner.assertConfigured(session), installed);
  assert.equal(calls.length, 3);
});

test('readback mismatch quarantines apparently successful setters', () => {
  const owner = new NativeSessionPolicies(),
    { session } = fixture();
  session._setOyaLocale = () => {};
  assert.throws(() => owner.configure(session, policy()), /installation failed/);
  assert.throws(() => owner.assertConfigured(session), /retire this session/);
});

test('reentrant exposure or configuration cannot observe an installing session as protected', () => {
  const owner = new NativeSessionPolicies(),
    { session } = fixture();
  const original = session._setOyaTimeZone;
  session._setOyaTimeZone = function (zone) {
    assert.throws(() => owner.assertConfigured(session), /retire this session/);
    assert.throws(() => owner.configure(session, policy()), /retire this session/);
    original.call(this, zone);
  };
  owner.configure(session, policy());
  assert.equal(owner.assertConfigured(session).timeZone, 'UTC');
});

for (const [field, value] of Object.entries({
  timeZone: 'Asia/Tokyo',
  locale: 'fr-FR',
  hardwareConcurrency: 4,
  acceptLanguages: 'fr-FR,fr',
  rendererStarted: true,
}))
  test('native readback disagreement in ' + field + ' never publishes a configured binding', () => {
    const owner = new NativeSessionPolicies(),
      { session, calls } = fixture();
    const read = session._getOyaSessionPolicy;
    session._getOyaSessionPolicy = function () {
      return calls.length ? { ...read.call(this), [field]: value } : read.call(this);
    };
    assert.throws(() => owner.configure(session, policy()), /installation failed/);
    assert.throws(() => owner.assertConfigured(session), /retire this session/);
  });

test('a native readback exception after setter completion permanently quarantines the session', () => {
  const owner = new NativeSessionPolicies(),
    { session, calls } = fixture();
  const read = session._getOyaSessionPolicy;
  session._getOyaSessionPolicy = function () {
    if (calls.length) throw Error('readback failed');
    return read.call(this);
  };
  assert.throws(() => owner.configure(session, policy()), /installation failed/);
  assert.throws(() => owner.configure(session, policy()), /retire this session/);
  assert.equal(calls.length, 3);
});
