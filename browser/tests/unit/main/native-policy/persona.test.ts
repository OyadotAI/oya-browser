/** Persona-derived native identity stays deterministic and refuses missing or contradictory protected inputs. */
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { nativePolicyForPersona, NativeSessionPolicies } from '../../../../src/main/native-policy/index.ts';

/** A real persona shape includes protections outside this explicit native subset. */
function persona(platform = 'MacIntel') {
  return {
    navigator: { platform, languages: ['en-US', 'en'], hardwareConcurrency: 8, userAgent: 'Untrusted/999' },
    timezone: 'America/New_York',
    locale: 'en-US',
    webgl: { renderer: 'Apple M2' },
    fonts: ['Arial'],
    screen: { width: 1440, height: 900 },
  };
}
const engine = 'Mozilla/5.0 Chrome/153.0.1234.56 Safari/537.36';

test('one deterministic policy binds persona platform, engine version and native metadata', () => {
  for (const [platform, hint, architecture] of [
    ['MacIntel', 'macOS', 'arm'],
    ['Win32', 'Windows', 'x86'],
    ['Linux x86_64', 'Linux', 'x86'],
  ]) {
    const policy = nativePolicyForPersona(persona(platform), engine);
    assert.deepEqual(policy, nativePolicyForPersona(persona(platform), engine));
    assert.equal(policy.platform, platform);
    assert.equal(policy.userAgentMetadata.platform, hint);
    assert.equal(policy.userAgentMetadata.architecture, architecture);
    assert.equal(policy.userAgentMetadata.fullVersion, '153.0.1234.56');
    assert.match(policy.userAgent, /Chrome\/153\.0\.0\.0/);
    assert.equal(policy.userAgentMetadata.brands.find((b) => b.brand === 'Chromium')?.version, '153');
    assert.equal(
      policy.userAgentMetadata.fullVersionList.find((b) => b.brand === 'Chromium')?.version,
      '153.0.1234.56',
    );
    assert.deepEqual(policy.userAgentMetadata.formFactors, ['Desktop']);
  }
});
test('missing and unknown platforms never inherit the host identity', () => {
  for (const platform of ['', 'Android', 'macOS'])
    assert.throws(() => nativePolicyForPersona(persona(platform), engine), /platform/);
  assert.throws(() => nativePolicyForPersona({}, engine), /object/);
});
test('engine version evidence is mandatory instead of using a fixed fallback', () => {
  for (const ua of [undefined, '', 'Chrome/999', 'Chrome/153.0.0.0junk'])
    assert.throws(() => nativePolicyForPersona(persona(), ua), /engine Chrome version/);
});
test('locale, languages, timezone and processors cannot silently lose requested identity', () => {
  const input = persona();
  input.navigator.languages = ['fr-FR', 'fr'];
  assert.throws(() => nativePolicyForPersona(input, engine), /languages/);
  input.navigator.languages = ['en-US', 'en', 'fr'];
  assert.throws(() => nativePolicyForPersona(input, engine), /languages/);
  input.navigator.languages = ['en-US'];
  assert.deepEqual(nativePolicyForPersona(input, engine).languages, ['en-US', 'en']);
  input.navigator.hardwareConcurrency = 0;
  assert.throws(() => nativePolicyForPersona(input, engine), /processor/);
  input.navigator.hardwareConcurrency = 8;
  input.timezone = 'Mars/Olympus';
  assert.throws(() => nativePolicyForPersona(input, engine));
});
test('policy snapshots are detached and deeply immutable', () => {
  const input = persona();
  const policy = nativePolicyForPersona(input, engine);
  input.navigator.languages[0] = 'fr';
  input.webgl.renderer = 'Intel';
  assert.equal(policy.languages[0], 'en-US');
  assert.equal(policy.userAgentMetadata.architecture, 'arm');
  assert.ok(Object.isFrozen(policy));
  assert.ok(Object.isFrozen(policy.userAgentMetadata.brands[0]));
});
test('persona preflight rejects invalid fields before any native setter runs', () => {
  let called = false;
  const session = {
    _setOyaUserAgent() {
      called = true;
    },
  };
  assert.throws(
    () => new NativeSessionPolicies().configurePersonaSubset(session, persona('Android'), engine),
    /platform/,
  );
  assert.equal(called, false);
});

test('host startup metadata uses the actual processor architecture without changing persona derivation', async () => {
  const { nativePolicyForHost } = await import('../../../../src/main/native-policy/index.ts');
  const input = persona();
  delete input.webgl;
  const host = nativePolicyForHost(input, engine);
  assert.equal(host.userAgentMetadata.architecture, process.arch === 'arm64' ? 'arm' : 'x86');
  assert.equal(nativePolicyForPersona(input, engine).userAgentMetadata.architecture, 'x86');
});
