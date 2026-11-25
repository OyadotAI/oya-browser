/** Signing authorization and publication stay ordered behind protected environments, approved refs and signature verification. */
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const yaml = require('js-yaml');
const { REQUIRED_ENVIRONMENT } = require('../../../build/windows-signing-constants.cjs');
/** Parse real workflow documents instead of matching formatting-sensitive snippets. */
function document(relative) {
  return yaml.load(fs.readFileSync(path.resolve(__dirname, '../../../../', relative), 'utf8'));
}
/** The local authentication action has the same entry in release and preflight jobs. */
const signingAction = './.github/actions/windows-signing';

test('release signing needs CI, a protected environment and a version-tag push', () => {
  const job = document('.github/workflows/deploy-prod.yaml').jobs['build-windows'];
  assert.equal(job.environment, 'windows-signing');
  assert.equal(job.needs, 'ci');
  assert.equal(job.if, "github.event_name == 'push' && startsWith(github.ref, 'refs/tags/v')");
  assert.equal(job.permissions['id-token'], 'write');
  const ancestry = job.steps.findIndex(step => step.run === 'node browser/build/verify-release-ref.cjs');
  const authenticate = job.steps.findIndex(step => step.uses === signingAction);
  const build = job.steps.findIndex(step => step.run?.includes('npm run dist:win'));
  assert.ok(ancestry >= 0 && ancestry < authenticate && authenticate < build);
  assert.match(job.steps[build].run, /--config build\/windows-signing\.cjs/);
  for (const name of REQUIRED_ENVIRONMENT) assert.equal(job.env[name], '${{ vars.' + name + ' }}');
});

test('all packaged and installed signature checks happen before release upload', () => {
  const job = document('.github/workflows/deploy-prod.yaml').jobs['build-windows'];
  const verify = job.steps.findIndex(step => step.name === 'Verify packaged and installed Windows signatures');
  const upload = job.steps.findIndex(step => step.name === 'Upload to GitHub release');
  assert.ok(verify >= 0 && verify < upload);
  assert.match(job.steps[verify].run, /-Files @\(\$installer, \$packaged\)/);
  assert.match(job.steps[verify].run, /-Files @\(\(Join-Path \$destination 'Oya Browser.exe'\)\)/);
  assert.match(job.steps[verify].run, /-ExpectedSubject \$env:WINDOWS_SIGNER_SUBJECT/);
});

test('preflight is restricted to main, needs offline Windows checks and cannot publish artifacts', () => {
  const workflow = document('.github/workflows/windows-signing-preflight.yaml');
  const native = workflow.jobs['native-checks'];
  const job = workflow.jobs['signing-configuration'];
  assert.equal(job.if, "github.ref == 'refs/heads/main'");
  assert.equal(job.needs, 'native-checks');
  assert.equal(job.environment, 'windows-signing');
  assert.deepEqual(job.permissions, { contents: 'read', 'id-token': 'write' });
  assert.deepEqual(native.permissions, { contents: 'read' });
  assert.equal(native.environment, undefined);
  assert.ok(job.steps.findIndex(step => step.uses === signingAction) < job.steps.findIndex(step => step.run?.includes('preflight-windows-signing.ps1')));
  for (const name of REQUIRED_ENVIRONMENT) assert.equal(job.env[name], '${{ vars.' + name + ' }}');
  assert.equal(job.steps.some(step => /upload|publish/.test(step.uses || step.run || '')), false);
});

test('Azure authentication uses a pinned action and profile-scoped tenant login without a credential secret', () => {
  const action = document('.github/actions/windows-signing/action.yml');
  const [validate, authenticate] = action.runs.steps;
  assert.match(validate.run, /windows-signing\.cjs/);
  assert.match(authenticate.uses, /^azure\/login@[a-f0-9]{40}$/);
  assert.equal(authenticate.with.audience, 'api://AzureADTokenExchange');
  assert.equal(authenticate.with['allow-no-subscriptions'], true);
  assert.equal(Object.hasOwn(authenticate.with, 'creds'), false);
  assert.equal(Object.hasOwn(authenticate.with, 'subscription-id'), false);
});
