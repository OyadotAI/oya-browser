/** Merge the existing desktop build with mandatory managed Windows signing and validated publisher configuration. */
const { REQUIRED_ENVIRONMENT, AZURE_SIGNING_DEFAULTS } = require('./windows-signing-constants.cjs');

/** Missing values name only the configuration key, never its contents. */
function requiredValue(environment, name) {
  const value = environment[name];
  if (typeof value !== 'string' || !value.trim()) throw Error(`Missing ${name}. Configure the windows-signing environment.`);
  if (/[\x00-\x1f\x7f]/.test(value)) throw Error(`Invalid ${name}: control characters are not allowed.`);
  return value.trim();
}
/** Azure identity identifiers must be UUIDs rather than substituted shell expressions. */
function validateIdentity(values) {
  const uuid = /^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/i;
  for (const name of ['AZURE_CLIENT_ID', 'AZURE_TENANT_ID']) {
    if (!uuid.test(values[name])) throw Error(`Invalid ${name}: expected an Azure identity UUID.`);
  }
}
/** A signer can contact only Microsoft's regional public-cloud signing endpoint. */
function validateService(values) {
  if (!/^https:\/\/[a-z0-9-]+\.codesigning\.azure\.net\/?$/.test(values.AZURE_SIGNING_ENDPOINT))
    throw Error('Invalid AZURE_SIGNING_ENDPOINT: use the account’s regional HTTPS signing endpoint.');
  for (const name of ['AZURE_SIGNING_ACCOUNT_NAME', 'AZURE_SIGNING_PROFILE_NAME']) {
    if (!/^[a-zA-Z0-9_-]+$/.test(values[name])) throw Error(`Invalid ${name}: use the Azure resource name.`);
  }
}
/** The signature guard requires an approved distinguished name, not a guessed product label. */
function validatePublisher(values) {
  if (!/^CN=.+/.test(values.WINDOWS_SIGNER_SUBJECT))
    throw Error('Invalid WINDOWS_SIGNER_SUBJECT: use the approved certificate’s exact subject.');
}
/** One validation path is shared by builds and the non-publishing signing preflight. */
function signingValues(environment) {
  const values = Object.fromEntries(REQUIRED_ENVIRONMENT.map((name) => [name, requiredValue(environment, name)]));
  validateIdentity(values);
  validateService(values);
  validatePublisher(values);
  return values;
}
/** Fixed algorithm and timestamp settings cannot be replaced through environment values. */
function azureOptions(values) {
  return {
    ...AZURE_SIGNING_DEFAULTS,
    publisherName: values.WINDOWS_PUBLISHER_NAME,
    endpoint: values.AZURE_SIGNING_ENDPOINT,
    codeSigningAccountName: values.AZURE_SIGNING_ACCOUNT_NAME,
    certificateProfileName: values.AZURE_SIGNING_PROFILE_NAME,
  };
}
/** Electron-builder supplies its context first; tests inject only the environment at the second seam. */
function windowsSigningConfiguration(_context, environment = process.env) {
  const values = signingValues(environment);
  const config = structuredClone(require('../package.json').build);
  delete config.win.signtoolOptions;
  config.win.azureSignOptions = azureOptions(values);
  config.win.forceCodeSigning = true;
  config.win.verifyUpdateCodeSignature = true;
  return config;
}
module.exports = windowsSigningConfiguration;
if (require.main === module) {
  windowsSigningConfiguration();
  console.log('Windows managed-signing configuration is valid.');
}
