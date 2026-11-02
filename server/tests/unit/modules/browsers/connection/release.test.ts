/** A retired registration cannot release a replacement's billing clock or persona slot. */
import assert from 'node:assert/strict';
import { afterEach, test, mock } from 'node:test';
import { releaseBrowser } from '../../../../../src/modules/browsers/connection/release.ts';
import { registry } from '../../../../../src/modules/browsers/registry.ts';
import { container } from '../../../../../src/app/container.ts';
import * as usage from '../../../../../src/platform/usage.ts';
import { connectBrowser, disconnectBrowser } from '../../../support/fakes.ts';

const ID = 'release-generation';
const KEY = 'release-key';
afterEach(() => {
  disconnectBrowser(ID);
  usage.browserDisconnected(KEY, ID);
  mock.restoreAll();
});

test('repeated cleanup of an old record cannot release a replacement', () => {
  connectBrowser(ID, KEY);
  const old = registry.get(ID);
  releaseBrowser(ID, old, 'disconnected');
  const replacement = connectBrowser(ID, KEY);
  usage.browserConnected(KEY, ID);
  const release = mock.method(container.personas, 'release', () => {});
  releaseBrowser(ID, old, 'disconnected');
  assert.equal(registry.get(ID)?.ws, replacement);
  assert.equal(usage.current(KEY).openBrowsers, 1);
  assert.equal(release.mock.callCount(), 0);
});

test('disconnect observers see the old resources released before registering a replacement', () => {
  connectBrowser(ID, KEY);
  const old = registry.get(ID);
  usage.browserConnected(KEY, ID);
  registry.once('browser:disconnected', () => {
    connectBrowser(ID, KEY);
    usage.browserConnected(KEY, ID);
  });
  releaseBrowser(ID, old, 'disconnected');
  assert.notEqual(registry.get(ID), old);
  assert.equal(usage.current(KEY).openBrowsers, 1);
});

test('a missing registration is already released', () => {
  assert.doesNotThrow(() => releaseBrowser(ID, undefined, 'disconnected'));
});
