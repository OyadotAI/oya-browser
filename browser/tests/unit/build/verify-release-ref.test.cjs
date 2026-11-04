/** Untrusted events, malformed identities and commits outside main must fail before any signing authentication. */
const { test } = require('node:test');
const assert = require('node:assert/strict');
const verify = require('../../../build/verify-release-ref.cjs');
/** Synthetic immutable release metadata never contacts GitHub or Azure. */
const release = { GITHUB_EVENT_NAME: 'push', GITHUB_REF: 'refs/tags/v1.2.3', GITHUB_SHA: '1'.repeat(40) };

test('a stable tag signs only after Git verifies its exact commit belongs to current main', () => {
  const calls = [];
  verify(release, (command, args) => calls.push({ command, args }));
  assert.equal(calls.length, 2);
  assert.deepEqual(calls[1], { command: 'git', args: ['merge-base', '--is-ancestor', release.GITHUB_SHA, 'refs/remotes/origin/main'] });
});

for (const metadata of [
  { GITHUB_EVENT_NAME: 'pull_request' },
  { GITHUB_EVENT_NAME: 'workflow_dispatch' },
  { GITHUB_REF: 'refs/heads/main' },
  { GITHUB_REF: 'refs/tags/unrelated' },
  { GITHUB_REF: 'refs/tags/v1.2.3/other' },
  { GITHUB_SHA: 'main; arbitrary command' },
]) {
  test(`untrusted release metadata is refused without invoking Git: ${JSON.stringify(metadata)}`, () => {
    let invoked = false;
    assert.throws(() => verify({ ...release, ...metadata }, () => { invoked = true; }));
    assert.equal(invoked, false);
  });
}

test('an unverifiable main branch or a release outside main refuses signing', () => {
  for (const failedStep of [0, 1]) {
    let step = 0;
    assert.throws(() => verify(release, () => { if (step++ === failedStep) throw Error('synthetic Git failure'); }), /refusing Windows signing/);
  }
});
