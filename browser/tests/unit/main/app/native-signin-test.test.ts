/** The isolated sign-in experiment cannot become a managed-policy or automation bypass. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import {
  startNativeSigninTest,
  nativeSigninTestEnabled,
  nativeSigninTestOptions,
  nativeSigninTestAllows,
} from '../../../../src/main/app/native-signin-test.ts';
const args = ['--oya-native-signin-test'];
it('does not affect ordinary launches', () => assert.equal(nativeSigninTestEnabled(false, [], {}), false));
it('requires explicit development opt-in', () => {
  assert.equal(nativeSigninTestEnabled(false, args, {}), true);
  assert.throws(() => nativeSigninTestEnabled(true, args, {}));
});
it('refuses managed, cloud and remote debugging environments', () => {
  for (const key of ['OYA_GOVERNANCE', 'OYA_DOCKER', 'OYA_REMOTE_DEBUGGING_PORT'])
    assert.throws(() => nativeSigninTestEnabled(false, args, { [key]: '1' }));
});
it('refuses debugging and automation arguments', () => {
  for (const arg of [
    '--remote-debugging-port=9222',
    '--remote-debugging-pipe',
    '--inspect=9229',
    '--enable-automation',
  ])
    assert.throws(() => nativeSigninTestEnabled(false, [...args, arg], {}));
});
it('uses a memory-only sandbox without preload or developer tools', () => {
  const p = nativeSigninTestOptions().webPreferences!;
  assert.equal(p.partition?.startsWith('persist:'), false);
  assert.equal(p.sandbox, true);
  assert.equal(p.contextIsolation, true);
  assert.equal(p.nodeIntegration, false);
  assert.equal(p.devTools, false);
  assert.equal(p.webSecurity, true);
  assert.equal(p.preload, undefined);
});
it('allows secure navigation but no local files, external apps or embedded credentials', () => {
  assert.equal(nativeSigninTestAllows('https://accounts.google.com/'), true);
  for (const url of [
    'file:///etc/passwd',
    'http://example.com',
    'javascript:alert(1)',
    'zoommtg://zoom.us',
    'https://user:pass@example.com',
    'invalid',
  ])
    assert.equal(nativeSigninTestAllows(url), false);
});

/** Fake only Electron's native window boundary; no browser or network is started. */
function diagnosticHarness() {
  const contents = new EventEmitter();
  const handlers: Record<string, (...args: unknown[]) => unknown> = {};
  const window = {
    webContents: Object.assign(contents, {
      session: {
        setPermissionRequestHandler: (fn: (...args: unknown[]) => unknown) => {
          handlers.request = fn;
        },
        setPermissionCheckHandler: (fn: (...args: unknown[]) => unknown) => {
          handlers.check = fn;
        },
      },
      setWindowOpenHandler: (fn: (...args: unknown[]) => unknown) => {
        handlers.popup = fn;
      },
    }),
    loadURL: async (url: string) => {
      loaded = url;
    },
    setTitle: (title: string) => {
      heading = title;
    },
  };
  let loaded = '',
    heading = '';
  const electron = {
    BrowserWindow: function () {
      return window;
    },
    Menu: { setApplicationMenu: () => {} },
  };
  startNativeSigninTest(electron as unknown as Parameters<typeof startNativeSigninTest>[0]);
  return { contents, handlers, loaded, heading: () => heading };
}

it('loads Gmail without permitting new page surfaces or permission grants', () => {
  const test = diagnosticHarness();
  assert.equal(test.loaded, 'https://mail.google.com/');
  assert.deepEqual(test.handlers.popup(), { action: 'deny' });
  assert.equal(test.handlers.check(), false);
  let allowed: unknown;
  test.handlers.request(null, 'media', (value: unknown) => {
    allowed = value;
  });
  assert.equal(allowed, false);
});
it('blocks unsafe navigations and redirects while keeping the actual origin visible', () => {
  const test = diagnosticHarness();
  for (const name of ['will-navigate', 'will-redirect']) {
    let prevented = false;
    test.contents.emit(
      name,
      {
        preventDefault: () => {
          prevented = true;
        },
      },
      'file:///etc/passwd',
    );
    assert.equal(prevented, true);
  }
  test.contents.emit('did-navigate', {}, 'https://accounts.google.com/path?private=value');
  assert.equal(test.heading(), 'Oya native diagnostic — https://accounts.google.com');
});

it('normal-window native browsing is an explicit development opt-in with the same isolation guards', () => {
  const flags = ['--oya-native-browsing'];
  assert.equal(nativeSigninTestEnabled(false, flags, {}), true);
  assert.throws(() => nativeSigninTestEnabled(true, flags, {}));
  assert.throws(() => nativeSigninTestEnabled(false, flags, { OYA_GOVERNANCE: '{}' }));
  assert.throws(() => nativeSigninTestEnabled(false, flags, { OYA_REMOTE_DEBUGGING_PORT: '9222' }));
});
