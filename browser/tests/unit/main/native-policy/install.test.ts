/** Native policy installation fails closed without mutating on invalid input or using a protocol fallback. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import identity from '../../../support/native-policy-identity.cjs';
import { NativeSessionPolicies } from '../../../../src/main/native-policy/index.ts';

/** Fresh caller-owned input lets each rule verify copying and mutation safety independently. */
function policy() {
  return {
    ...identity(),
    platform: 'Win32',
    timeZone: 'UTC',
    locale: 'en-US',
    hardwareConcurrency: 8,
    languages: ['en-US', 'en'],
  };
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
    platform: undefined as string | undefined,
    userAgent: '',
    userAgentMetadata: undefined as unknown,
  };
  const session = {
    _getOyaSessionPolicy() {
      assert.equal(this, session);
      const { platform, userAgentMetadata, ...base } = state;
      const rest = userAgentMetadata === undefined ? base : { ...base, userAgentMetadata };
      return platform === undefined ? rest : { ...rest, platform };
    },
    _setOyaTimeZone(value: string) {
      step(this, 'zone');
      state.timeZone = value;
    },
    _setOyaHardwareConcurrency(value: number) {
      step(this, 'cores');
      state.hardwareConcurrency = value;
    },
    _setOyaUserAgent(value: string) {
      step(this, 'ua');
      state.userAgent = value;
    },
    _setOyaUserAgentMetadata(value: unknown) {
      step(this, 'metadata');
      state.userAgentMetadata = structuredClone(value);
    },
    _setOyaPlatform(value: string) {
      step(this, 'platform');
      state.platform = value;
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
  assert.deepEqual(calls, ['zone', 'cores', 'locale', 'platform', 'ua', 'metadata']);
  assert.equal(owner.assertConfigured(session), result);
  assert.equal(owner.configure(session, policy()), result);
  assert.deepEqual(calls, ['zone', 'cores', 'locale', 'platform', 'ua', 'metadata']);
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
  ...[undefined, null, 1, true, {}, [], '', 'macintel', 'Windows', 'Linux', 'Win32\0hidden'].map((platform) => ({
    ...policy(),
    platform,
  })),
  null,
  [],
  {},
  { ...policy(), unsupported: true },
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

for (const method of [
  '_getOyaSessionPolicy',
  '_setOyaTimeZone',
  '_setOyaHardwareConcurrency',
  '_setOyaLocale',
  '_setOyaPlatform',
  '_setOyaUserAgent',
  '_setOyaUserAgentMetadata',
])
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

for (const stage of ['zone', 'cores', 'locale', 'platform', 'ua', 'metadata'])
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
  assert.equal(calls.length, 6);
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
  userAgent: 'Other/1',
  userAgentMetadata: { invalid: true },
  platform: 'MacIntel',
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
  assert.equal(calls.length, 6);
});

for (const platform of ['MacIntel', 'Win32', 'Linux x86_64'])
  test('canonical native platform ' + platform + ' is retained as an immutable snapshot', () => {
    const owner = new NativeSessionPolicies(),
      { session, calls } = fixture();
    const input = { ...policy(), ...identity(platform), platform };
    const result = owner.configure(session, input);
    input.platform = 'Changed by caller';
    assert.equal(result.platform, platform);
    assert.ok(Object.isFrozen(result));
    assert.equal(owner.configure(session, { ...policy(), ...identity(platform), platform }), result);
    assert.equal(calls.length, 6);
    assert.throws(
      () => owner.configure(session, { ...policy(), platform: platform === 'Win32' ? 'MacIntel' : 'Win32' }),
      /cannot change/,
    );
    assert.equal(owner.assertConfigured(session), result);
  });

test('missing platform cannot certify an implicit host identity', () => {
  const { platform, ...input } = policy();
  const { session, calls } = fixture();
  assert.equal(platform, 'Win32');
  assert.throws(() => new NativeSessionPolicies().configure(session, input));
  assert.deepEqual(calls, []);
});

for (const platform of [undefined, null, '', 'Windows', 'Win32\0hidden', 1])
  test('malformed native platform readback ' + String(platform) + ' fails before mutation', () => {
    const { session, calls } = fixture();
    const read = session._getOyaSessionPolicy;
    (session as any)._getOyaSessionPolicy = () => ({ ...read.call(session), platform });
    assert.throws(() => new NativeSessionPolicies().configure(session, policy()), /Unsupported native policy readback/);
    assert.deepEqual(calls, []);
  });

test('a platform setter that silently does nothing permanently quarantines the session', () => {
  const owner = new NativeSessionPolicies(),
    { session, calls } = fixture();
  session._setOyaPlatform = () => {};
  assert.throws(() => owner.configure(session, policy()), /installation failed/);
  assert.throws(() => owner.assertConfigured(session), /retire this session/);
  assert.throws(() => owner.configure(session, policy()), /retire this session/);
  assert.deepEqual(calls, ['zone', 'cores', 'locale', 'ua', 'metadata']);
});

const badMetadata = [
  undefined,
  null,
  true,
  [],
  {},
  { ...identity().userAgentMetadata, unknown: true },
  { ...identity().userAgentMetadata, [Symbol('hidden')]: true },
  ...['mobile', 'wow64'].map((key) => ({ ...identity().userAgentMetadata, [key]: 1 })),
  ...['fullVersion', 'platform'].map((key) => ({ ...identity().userAgentMetadata, [key]: '' })),
  ...['fullVersion', 'platform', 'platformVersion', 'architecture', 'model', 'bitness'].flatMap((key) =>
    [null, 1, 'x\0y', 'é', 'x'.repeat(129)].map((value) => ({ ...identity().userAgentMetadata, [key]: value })),
  ),
  ...['brands', 'fullVersionList'].flatMap((key) =>
    [
      null,
      [],
      new Array(1),
      Array(9).fill({ brand: 'x', version: '1' }),
      [{ brand: '', version: '1' }],
      [{ brand: 'x', version: '' }],
      [{ brand: 'x', version: 1 }],
      [{ brand: 'x', version: '1', extra: true }],
      [
        { brand: 'x', version: '1' },
        { brand: 'x', version: '2' },
      ],
      [{ brand: 'Mismatch', version: '1' }],
      [
        { brand: 'x', version: '1' },
        { brand: 'y', version: '2' },
      ],
    ].map((value) => ({ ...identity().userAgentMetadata, [key]: value })),
  ),
  ...[null, [], new Array(1), Array(8).fill('Desktop'), ['Invalid'], ['Desktop', 'Desktop'], [1]].map(
    (formFactors) => ({ ...identity().userAgentMetadata, formFactors }),
  ),
];
for (const [index, userAgentMetadata] of badMetadata.entries())
  test('invalid metadata case ' + index + ' never starts native installation', () => {
    const { session, calls } = fixture();
    assert.throws(() => new NativeSessionPolicies().configure(session, { ...policy(), userAgentMetadata }));
    assert.deepEqual(calls, []);
  });

for (const [index, userAgent] of [
  undefined,
  null,
  true,
  1,
  '',
  'x\0y',
  'x\ry',
  'x\ny',
  'é',
  'x'.repeat(1025),
].entries())
  test('invalid UA case ' + index + ' never starts native installation', () => {
    const { session, calls } = fixture();
    assert.throws(() => new NativeSessionPolicies().configure(session, { ...policy(), userAgent }));
    assert.deepEqual(calls, []);
  });

test('metadata snapshot deeply freezes copies and ignores input property ordering on reuse', () => {
  const owner = new NativeSessionPolicies(),
    { session } = fixture(),
    input = policy();
  const result = owner.configure(session, input);
  input.userAgentMetadata.brands[0].brand = 'Caller mutation';
  input.userAgentMetadata.fullVersionList[0].version = '9';
  input.userAgentMetadata.formFactors.push('Watch');
  assert.deepEqual(result.userAgentMetadata, owner.assertConfigured(session).userAgentMetadata);
  for (const value of [
    result.userAgentMetadata,
    result.userAgentMetadata.brands,
    result.userAgentMetadata.brands[0],
    result.userAgentMetadata.fullVersionList,
    result.userAgentMetadata.fullVersionList[0],
    result.userAgentMetadata.formFactors,
  ])
    assert.ok(Object.isFrozen(value));
  assert.equal(result.userAgentMetadata.brands[0].brand, identity().userAgentMetadata.brands[0].brand);
  const reversed = Object.fromEntries(Object.entries(identity().userAgentMetadata).reverse());
  assert.equal(owner.configure(session, { ...policy(), userAgentMetadata: reversed }), result);
});

test('native metadata readback field order does not falsely retire a matching session', () => {
  const { session, state } = fixture();
  const read = session._getOyaSessionPolicy;
  session._getOyaSessionPolicy = function () {
    const value = read.call(this);
    return state.userAgentMetadata
      ? { ...value, userAgentMetadata: Object.fromEntries(Object.entries(state.userAgentMetadata as object).reverse()) }
      : value;
  };
  assert.equal(new NativeSessionPolicies().configure(session, policy()).userAgent, identity().userAgent);
});

for (const field of ['userAgent', 'userAgentMetadata'])
  test('native ' + field + ' no-op setter cannot expose a partially installed identity', () => {
    const owner = new NativeSessionPolicies(),
      { session } = fixture();
    if (field === 'userAgent') session._setOyaUserAgent = () => {};
    else session._setOyaUserAgentMetadata = () => {};
    assert.throws(() => owner.configure(session, policy()), /installation failed/);
    assert.throws(() => owner.assertConfigured(session), /retire this session/);
  });

test('all native metadata bounds are accepted without retaining input arrays', () => {
  const { session } = fixture(),
    input = policy();
  input.userAgent = 'x'.repeat(1024);
  input.userAgentMetadata.model = 'x'.repeat(128);
  input.userAgentMetadata.brands = Array.from({ length: 8 }, (_, i) => ({ brand: 'Brand' + i, version: '1' }));
  input.userAgentMetadata.fullVersionList = input.userAgentMetadata.brands.map(({ brand }) => ({
    brand,
    version: '1.0.0.0',
  }));
  input.userAgentMetadata.formFactors = ['Desktop', 'Automotive', 'Mobile', 'Tablet', 'XR', 'EInk', 'Watch'];
  const result = new NativeSessionPolicies().configure(session, input);
  assert.deepEqual(result.userAgentMetadata.formFactors, input.userAgentMetadata.formFactors);
  assert.notEqual(result.userAgentMetadata.formFactors, input.userAgentMetadata.formFactors);
});

for (const field of ['userAgent', 'userAgentMetadata'])
  test('changed ' + field + ' cannot repurpose an installed native session', () => {
    const owner = new NativeSessionPolicies(),
      { session, calls } = fixture();
    const installed = owner.configure(session, policy());
    const input = policy();
    if (field === 'userAgent') input.userAgent = 'Different/2';
    else input.userAgentMetadata.formFactors = ['Tablet'];
    assert.throws(() => owner.configure(session, input), /cannot change/);
    assert.equal(owner.assertConfigured(session), installed);
    assert.equal(calls.length, 6);
  });

for (const value of [undefined, null, 'x\0y'])
  test('malformed UA native readback ' + String(value) + ' fails before any setter', () => {
    const { session, calls } = fixture();
    const read = session._getOyaSessionPolicy;
    (session as any)._getOyaSessionPolicy = () => ({ ...read.call(session), userAgent: value });
    assert.throws(() => new NativeSessionPolicies().configure(session, policy()));
    assert.deepEqual(calls, []);
  });

test('malformed existing native metadata is rejected before any setter', () => {
  const { session, calls } = fixture();
  const read = session._getOyaSessionPolicy;
  session._getOyaSessionPolicy = () => ({ ...read.call(session), userAgentMetadata: {} });
  assert.throws(() => new NativeSessionPolicies().configure(session, policy()));
  assert.deepEqual(calls, []);
});
