/**
 * Unit tests for the front door's guard: every client opens web addresses
 * only and cannot reach Chromium around the door; a relay also cannot reach
 * this computer's files, the app's life or its permissions.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { answerFor } = require('../../../front-door/guard.cjs');
const { NOT_A_WEB_ADDRESS } = require('../../../main/tabs/navigation.cjs');
const { LOCAL_FILES_UNAVAILABLE, REMOTE_UNAVAILABLE, AROUND_THE_DOOR } = require('../../../constants.cjs');

/** A command with `method` and `params`. */
const cmd = (method, params) => ({ id: 1, method, params });

describe('front door guard', () => {
  it('refuses file:, javascript:, chrome: and data: navigations and new tabs, for every client', () => {
    for (const url of [
      'file:///etc/hosts',
      'javascript:alert(1)',
      'chrome://settings',
      'data:text/html,x',
      ' FILE:///x',
    ]) {
      for (const relay of [false, true]) {
        assert.deepEqual(answerFor(cmd('Page.navigate', { url }), relay), { error: NOT_A_WEB_ADDRESS }, url);
        assert.deepEqual(answerFor(cmd('Target.createTarget', { url }), relay), { error: NOT_A_WEB_ADDRESS }, url);
      }
    }
  });

  it('lets http(s), about:blank and a createTarget with no url through', () => {
    for (const url of ['https://a.test/', 'HTTP://b.test', 'about:blank']) {
      assert.equal(answerFor(cmd('Page.navigate', { url }), true), null, url);
      assert.equal(answerFor(cmd('Target.createTarget', { url }), true), null, url);
    }
    assert.equal(answerFor(cmd('Target.createTarget', {}), true), null);
  });

  it('refuses a navigation whose url is not a string, rather than reading it as one', () => {
    assert.deepEqual(answerFor(cmd('Page.navigate', { url: ['file:///x'] }), false), { error: NOT_A_WEB_ADDRESS });
    assert.deepEqual(answerFor(cmd('Page.navigate', {}), false), { error: NOT_A_WEB_ADDRESS });
  });

  it('refuses, for every client, the commands that reach Chromium around the door', () => {
    for (const method of ['Target.exposeDevToolsProtocol', 'Target.sendMessageToTarget']) {
      assert.deepEqual(answerFor(cmd(method, {}), false), { error: AROUND_THE_DOOR }, method);
    }
  });

  it('refuses local-file methods for a relay only', () => {
    const files = [
      cmd('DOM.setFileInputFiles', { files: ['/etc/passwd'] }),
      cmd('Input.dispatchDragEvent', { type: 'drop', data: { items: [], files: ['/etc/passwd'] } }),
      cmd('Page.setDownloadBehavior', { behavior: 'allow', downloadPath: '/Users/x' }),
    ];
    for (const msg of files) {
      assert.deepEqual(answerFor(msg, true), { error: LOCAL_FILES_UNAVAILABLE }, msg.method);
      assert.equal(answerFor(msg, false), null, msg.method);
    }
  });

  it('lets a relay drag without files', () => {
    assert.equal(answerFor(cmd('Input.dispatchDragEvent', { type: 'dragEnter', data: { items: [] } }), true), null);
  });

  it('refuses a relay the commands that reach the computer, not the page', () => {
    const methods = [
      'Browser.close',
      'Browser.crash',
      'Browser.crashGpuProcess',
      'Browser.grantPermissions',
      'Browser.setPermission',
      'Tethering.bind',
    ];
    for (const method of methods) {
      assert.deepEqual(answerFor(cmd(method, {}), true), { error: REMOTE_UNAVAILABLE }, method);
      assert.equal(answerFor(cmd(method, {}), false), null, method);
    }
  });

  it('stubs Browser.setDownloadBehavior for a relay, so Playwright connects and the caller names no folder', () => {
    assert.deepEqual(answerFor(cmd('Browser.setDownloadBehavior', { behavior: 'allow', downloadPath: '/tmp' }), true), {
      result: {},
    });
    assert.equal(answerFor(cmd('Browser.setDownloadBehavior', {}), false), null);
  });

  it('forwards everything else, including a second browser session and a method named like an Object prototype key', () => {
    for (const method of [
      'Target.attachToBrowserTarget',
      'Runtime.evaluate',
      'Page.captureScreenshot',
      'toString',
      'constructor',
      '__proto__',
    ]) {
      assert.equal(answerFor(cmd(method, {}), true), null, method);
    }
  });
});
