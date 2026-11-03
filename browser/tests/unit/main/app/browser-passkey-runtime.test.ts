/** The isolated experimental path requires the exact native runtime protocol and never enables itself on stock Electron. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { createBrowserPasskeys } from '../../../../src/main/app/browser-passkey-runtime.ts';
import { BROWSER_PASSKEY_SWITCH } from '../../../../src/main/app/constants.ts';

it('ordinary stock desktop launches install no browser-native strategy', () => {
  assert.equal(createBrowserPasskeys({ electron: { app: {} } } as any, [], 'darwin'), undefined);
});

it('explicit opt-in refuses missing, disabled, wrong-version and other-platform native capabilities', () => {
  for (const capability of [undefined, { protocolVersion: 0, enabled: true }, { protocolVersion: 1, enabled: false }]) {
    const deps = { electron: { app: { getOyaBrowserPasskeyCapabilities: () => capability } } } as any;
    assert.throws(
      () => createBrowserPasskeys(deps, [BROWSER_PASSKEY_SWITCH], 'darwin'),
      /verified matching test build/,
    );
  }
  const deps = {
    electron: { app: { getOyaBrowserPasskeyCapabilities: () => ({ protocolVersion: 1, enabled: true }) } },
  } as any;
  assert.throws(() => createBrowserPasskeys(deps, [BROWSER_PASSKEY_SWITCH], 'win32'));
  assert.ok(createBrowserPasskeys(deps, [BROWSER_PASSKEY_SWITCH], 'darwin'));
});

it('a failed native capability probe reports a fixed unsupported-build error', () => {
  const deps = {
    electron: {
      app: {
        getOyaBrowserPasskeyCapabilities: () => {
          throw Error('fixture-sensitive-detail');
        },
      },
    },
  } as any;
  assert.throws(
    () => createBrowserPasskeys(deps, [BROWSER_PASSKEY_SWITCH], 'darwin'),
    (error: unknown) =>
      error instanceof Error &&
      /verified matching test build/.test(error.message) &&
      !error.message.includes('fixture-sensitive-detail'),
  );
});
