/** Require a stable release tag whose exact immutable commit belongs to the approved main branch before Azure authentication. */
const { execFileSync } = require('node:child_process');

/** Arguments remain separate and only GitHub's validated immutable SHA reaches the ancestry command. */
function verifyReleaseRef(environment = process.env, execute = execFileSync) {
  if (environment.GITHUB_EVENT_NAME !== 'push' || !/^refs\/tags\/v\d+\.\d+\.\d+$/.test(environment.GITHUB_REF || ''))
    throw Error('Windows release signing requires a stable version-tag push.');
  if (!/^[a-f0-9]{40}$/i.test(environment.GITHUB_SHA || '')) throw Error('Windows release signing requires an immutable commit SHA.');
  verifyAncestry(environment.GITHUB_SHA, execute);
}
/** Failure to read main or establish ancestry refuses signing instead of trusting the tag name. */
function verifyAncestry(sha, execute) {
  try {
    execute('git', ['fetch', '--no-tags', 'origin', '+refs/heads/main:refs/remotes/origin/main'], { stdio: 'pipe' });
    execute('git', ['merge-base', '--is-ancestor', sha, 'refs/remotes/origin/main'], { stdio: 'pipe' });
  } catch {
    throw Error('Cannot verify the release commit belongs to main; refusing Windows signing.');
  }
}
module.exports = verifyReleaseRef;
if (require.main === module) {
  verifyReleaseRef();
  console.log('Windows release tag points to an approved main-branch commit.');
}
