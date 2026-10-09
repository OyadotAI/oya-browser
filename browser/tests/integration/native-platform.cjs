/** Verify native legacy platform policy independently from UA metadata, with no page getter replacement. */
const assert = require('node:assert/strict');
const enabled = process.env.OYA_CHECK_NATIVE_PLATFORM === '1';
const values = ['Win32', 'Linux x86_64', 'MacIntel'];
/** Exact desktop tokens are immutable; invalid input cannot install partial native state. */
function configure(jar, index) {
  if (!enabled) return;
  for (const value of [
    '',
    'macintel',
    'Windows',
    'Linux',
    'MacIntel\0hidden',
    'Win32\r\n',
    'x'.repeat(129),
    null,
    true,
    1,
    {},
  ]) {
    assert.throws(() => jar._setOyaPlatform(value));
    assert.equal(jar._getOyaSessionPolicy().platform, undefined);
  }
  jar._setOyaPlatform(values[index]);
  frozen(jar, index);
  const snapshot = jar._getOyaSessionPolicy();
  snapshot.platform = 'CallerMutation';
  assert.equal(jar._getOyaSessionPolicy().platform, values[index]);
}
/** An identical value remains valid after startup, but a different platform never does. */
function frozen(jar, index) {
  if (!enabled) return;
  jar._setOyaPlatform(values[index]);
  assert.throws(() => jar._setOyaPlatform(values[(index + 1) % values.length]), /cannot be changed/);
  assert.equal(jar._getOyaSessionPolicy().platform, values[index]);
}
/** Late installation stays forbidden even when the session's original window has been destroyed. */
function late(jar) {
  if (enabled) assert.throws(() => jar._setOyaPlatform('Win32'), /before any session renderer/);
}
/** Platform-only sessions must refuse spare renderer reuse without needing another installed policy. */
async function lifecycle({ session, windowFor, snapshots, url }) {
  if (!enabled) return;
  const baseline = windowFor(session.fromPartition('platform-baseline'));
  const host = await snapshots(baseline, url);
  const expectedHost = { darwin: 'MacIntel', win32: 'Win32', linux: 'Linux x86_64' }[process.platform];
  assert.equal(host.page.platform, expectedHost, 'unconfigured host ignores forged arguments');
  baseline.destroy();
  for (const [index, value] of values.entries()) {
    const jar = session.fromPartition('platform-only-' + index);
    configure(jar, index);
    const window = windowFor(jar);
    for (const [surface, actual] of Object.entries(await snapshots(window, url)))
      assert.deepEqual(actual, { ...host[surface], platform: value }, surface + ' platform only');
    frozen(jar, index);
    window.destroy();
    late(session.fromPartition('platform-baseline'));
    await jar.serviceWorkers._stopAllWorkers();
    await jar.serviceWorkers.startWorkerForScope(url);
    const resumed = windowFor(jar);
    await resumed.loadURL(url);
    assert.deepEqual(await resumed.webContents.executeJavaScript('fetch("/worker-first").then(r=>r.json())'), {
      ...host.service,
      platform: value,
    });
    resumed.destroy();
  }
  console.log(
    'PASS native platform-only sessions: all supported values, host preservation, forged arguments, first scripts, all workers and cold service restart',
  );
}
/** Add platform expectations only for the explicit experimental engine fixture. */
const expected = (index) => (enabled ? { platform: values[index] } : {});
const snapshot = enabled ? ',platform:navigator.platform' : '';
const additionalArguments = enabled ? ['--oya-session-platform=Linux x86_64', '--oya-session-platform=Win32'] : [];
module.exports = { enabled, configure, frozen, late, lifecycle, expected, snapshot, additionalArguments };
