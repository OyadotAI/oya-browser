/** Default-browser requests are opt-in and never mistake registration for OS confirmation. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { DefaultBrowser } from '../../../../src/main/app/default-browser.ts';

/** All operating-system effects are faked, never applied to the test machine. */
function setup(platform = 'darwin', packaged = true, current = false) {
  const calls = [],
    messages = [],
    external = [];
  const app = {
    isPackaged: packaged,
    isDefaultProtocolClient: () => current,
    setAsDefaultProtocolClient: (scheme) => {
      calls.push(scheme);
      return true;
    },
  };
  const deps = {
    electron: {
      app,
      shell: {
        openExternal: async (url) => {
          external.push(url);
        },
      },
      dialog: {
        showMessageBox: async (options) => {
          messages.push(options);
        },
      },
    },
    shell: { window: null },
  };
  return { service: new DefaultBrowser(deps as any, platform), app, calls, messages, external };
}
it('does nothing until explicitly requested and refuses development registration', async () => {
  const test = setup('darwin', false);
  assert.deepEqual(test.calls, []);
  await test.service.request();
  assert.deepEqual(test.calls, []);
  assert.equal(test.messages[0].message, 'Install Oya Browser first');
});
it('checks both web schemes before declaring the browser default', () => {
  const test = setup();
  test.app.isDefaultProtocolClient = (scheme) => scheme === 'https';
  assert.equal(test.service.isDefault(), false);
});
it('requests both schemes but does not claim success based on registration alone', async () => {
  const test = setup();
  await test.service.request();
  assert.deepEqual(test.calls, ['http', 'https']);
  assert.equal(test.messages[0].message, 'Finish in system settings');
});
it('opens Windows Default Apps without rewriting web defaults', async () => {
  const test = setup('win32');
  await test.service.request();
  assert.deepEqual(test.external, ['ms-settings:defaultapps']);
  assert.deepEqual(test.calls, []);
});
it('leaves an already-default browser alone', async () => {
  const test = setup('darwin', true, true);
  await test.service.request();
  assert.deepEqual(test.calls, []);
  assert.equal(test.messages[0].message, 'Oya is your default browser');
});
it('reports native failures without claiming the default changed', async () => {
  const test = setup();
  test.app.setAsDefaultProtocolClient = () => {
    throw new Error('OS refused');
  };
  await test.service.request();
  assert.equal(test.messages[0].message, 'Default browser was not changed');
});
