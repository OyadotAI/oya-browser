/** Approved Azure signing inputs and fixed cryptographic defaults for Windows releases. */
/** Both authentication and publisher metadata must exist before signing can start. */
const REQUIRED_ENVIRONMENT = [
  'AZURE_CLIENT_ID',
  'AZURE_TENANT_ID',
  'AZURE_SIGNING_ENDPOINT',
  'AZURE_SIGNING_ACCOUNT_NAME',
  'AZURE_SIGNING_PROFILE_NAME',
  'WINDOWS_PUBLISHER_NAME',
  'WINDOWS_SIGNER_SUBJECT',
];
/** Microsoft supplies the timestamp independently of the short-lived signing certificate. */
const TIMESTAMP_ENDPOINT = 'http://timestamp.acs.microsoft.com';
/** Every signed digest uses the current release algorithm without caller overrides. */
const SIGNING_DIGEST = 'SHA256';
/** Cryptographic settings are fixed independently of publisher input. */
const AZURE_SIGNING_DEFAULTS = {
  fileDigest: SIGNING_DIGEST,
  timestampDigest: SIGNING_DIGEST,
  timestampRfc3161: TIMESTAMP_ENDPOINT,
};
module.exports = { REQUIRED_ENVIRONMENT, AZURE_SIGNING_DEFAULTS };
