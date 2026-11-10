/** The two store manifests: the same extension, differing only by Firefox's settings block. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { GECKO_ID, manifestFor } from '../../scripts/pack.mjs';

const base = JSON.parse(readFileSync(new URL('../../src/manifest.base.json', import.meta.url), 'utf8'));

test('the Chrome and Firefox manifests differ only by browser_specific_settings', () => {
  const { browser_specific_settings: gecko, ...firefox } = manifestFor('firefox', base, '1.2.3');
  assert.deepEqual(firefox, manifestFor('chrome', base, '1.2.3'));
  assert.equal(gecko.gecko.id, GECKO_ID);
});

test('the manifest asks only for Oya up front: every other site is optional, asked at use', () => {
  const manifest = manifestFor('chrome', base, '1.2.3');
  assert.deepEqual(manifest.host_permissions, ['https://oyabrowser.com/*']);
  assert.deepEqual(manifest.optional_host_permissions, ['*://*/*']);
  assert.equal(manifest.background, undefined);
});

test('Firefox declares the logins it sends', () => {
  const { gecko } = manifestFor('firefox', base, '1.2.3').browser_specific_settings;
  assert.deepEqual(gecko.data_collection_permissions.required, ['authenticationInfo']);
});
