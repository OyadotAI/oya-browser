/** Faithful native policy seam for lifecycle and fail-closed protection tests. */
import assert from 'node:assert/strict';
/** The native seam records ordering and enforces receiver identity, with injected failure points. */
export function nativeProtectionFixture(fail = '') {
  const calls: string[] = [];
  const state = {
    version: 1,
    preScriptPolicyVersion: 1,
    preScriptPolicy: undefined as unknown,
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
    getUserAgent() {
      return 'Chrome/152.0.7977.130';
    },
    _setOyaPreScriptPolicy(value: unknown) {
      step(this, 'scripts');
      state.preScriptPolicy = structuredClone(value);
    },
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
