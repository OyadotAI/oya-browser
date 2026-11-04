/** Managed signing preserves packaging and rejects missing or unsafe identity configuration before any external operation. */
const { test } = require('node:test');
const assert = require('node:assert/strict');
const configuration = require('../../../build/windows-signing.cjs');
const { REQUIRED_ENVIRONMENT } = require('../../../build/windows-signing-constants.cjs');
const base = require('../../../package.json').build;

/** Synthetic publisher and identity inputs never authenticate or call a signing service. */
function environment(overrides = {}) {
  return {
    AZURE_CLIENT_ID: '11111111-1111-1111-1111-111111111111',
    AZURE_TENANT_ID: '22222222-2222-2222-2222-222222222222',
    AZURE_SIGNING_ENDPOINT: 'https://eus.codesigning.azure.net',
    AZURE_SIGNING_ACCOUNT_NAME: 'fixture-account',
    AZURE_SIGNING_PROFILE_NAME: 'fixture-profile',
    WINDOWS_PUBLISHER_NAME: 'Fixture Publisher',
    WINDOWS_SIGNER_SUBJECT: 'CN=Fixture Publisher, O=Fixture Publisher, C=US',
    ...overrides,
  };
}

test('the Azure build retains packaging, installer and macOS settings without mutating package configuration', () => {
  const before = structuredClone(base);
  const result = configuration(undefined, environment());
  const { win, ...unchanged } = result;
  const { win: originalWin, ...original } = base;
  assert.deepEqual(unchanged, original);
  assert.deepEqual(base, before);
  assert.deepEqual(win.target, originalWin.target);
  assert.equal(win.artifactName, originalWin.artifactName);
  assert.equal(win.icon, originalWin.icon);
  assert.equal(win.signtoolOptions, undefined);
  assert.equal(win.forceCodeSigning, true);
  assert.equal(win.verifyUpdateCodeSignature, true);
});

test('the managed provider receives only approved publisher metadata and fixed timestamped SHA256 options', () => {
  assert.deepEqual(configuration(undefined, environment()).win.azureSignOptions, {
    publisherName: 'Fixture Publisher',
    endpoint: 'https://eus.codesigning.azure.net',
    codeSigningAccountName: 'fixture-account',
    certificateProfileName: 'fixture-profile',
    fileDigest: 'SHA256',
    timestampDigest: 'SHA256',
    timestampRfc3161: 'http://timestamp.acs.microsoft.com',
  });
});

for (const name of REQUIRED_ENVIRONMENT) {
  test(`missing ${name} refuses signing without an unsigned fallback`, () => {
    assert.throws(() => configuration(undefined, environment({ [name]: '' })), new RegExp(`Missing ${name}`));
  });
}

for (const endpoint of ['http://eus.codesigning.azure.net', 'https://signing.invalid', 'https://eus.codesigning.azure.net.invalid', 'https://user@eus.codesigning.azure.net', 'https://eus.codesigning.azure.net/path', 'https://eus.codesigning.azure.net?token=fixture']) {
  test(`an unrelated or modified signing endpoint is refused: ${endpoint}`, () => {
    assert.throws(() => configuration(undefined, environment({ AZURE_SIGNING_ENDPOINT: endpoint })), /Invalid AZURE_SIGNING_ENDPOINT/);
  });
}

test('invalid Azure identifiers and resource names are refused before builder invocation', () => {
  for (const name of ['AZURE_CLIENT_ID', 'AZURE_TENANT_ID', 'AZURE_SIGNING_ACCOUNT_NAME', 'AZURE_SIGNING_PROFILE_NAME']) {
    assert.throws(() => configuration(undefined, environment({ [name]: 'invalid; expression' })), new RegExp(`Invalid ${name}`));
  }
});

test('publisher metadata must use a certificate subject and cannot insert a second log line', () => {
  assert.throws(() => configuration(undefined, environment({ WINDOWS_SIGNER_SUBJECT: 'Product label' })), /Invalid WINDOWS_SIGNER_SUBJECT/);
  for (const name of ['WINDOWS_PUBLISHER_NAME', 'WINDOWS_SIGNER_SUBJECT']) {
    assert.throws(() => configuration(undefined, environment({ [name]: 'CN=Fixture\nAnother line' })), /control characters/);
  }
});

test('legal publisher punctuation is kept as data rather than interpreted as options', () => {
  const publisher = "Fixture's Software, Inc.";
  const result = configuration(undefined, environment({ WINDOWS_PUBLISHER_NAME: publisher, WINDOWS_SIGNER_SUBJECT: `CN="${publisher}", C=US` }));
  assert.equal(result.win.azureSignOptions.publisherName, publisher);
});
