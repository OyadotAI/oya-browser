/**
 * The Claude plugin bundle lives in plugins/oya-browser, apart from the repo
 * root, because Anthropic's directory scans the whole plugin folder and refuses
 * any file over 5 MiB (the demo videos in assets/ are larger). These tests keep
 * the bundle and what points at it consistent.
 */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';

/** Where the bundle lives, relative to the repo root the tests run from. */
const BUNDLE = 'plugins/oya-browser';

/** A repo file as text. */
const read = (path) => readFileSync(path, 'utf8');

test('the marketplace points its oya-browser plugin at the bundle folder', () => {
  const plugin = JSON.parse(read('.claude-plugin/marketplace.json')).plugins.find((p) => p.name === 'oya-browser');
  assert.equal(plugin.source, `./${BUNDLE}`);
});

test('the bundle has its manifest and the root no longer does', () => {
  assert.equal(JSON.parse(read(`${BUNDLE}/.claude-plugin/plugin.json`)).name, 'oya-browser');
  assert.equal(existsSync('.claude-plugin/plugin.json'), false);
});

test('the bundle carries the same skill as skills/oya-browser, which npx skills installs', () => {
  assert.equal(read(`${BUNDLE}/skills/oya-browser/SKILL.md`), read('skills/oya-browser/SKILL.md'));
});

test('the release script stamps the version into the bundle manifest', () => {
  assert.match(read('k8s/scripts/release.sh'), new RegExp(`${BUNDLE}/\\.claude-plugin/plugin\\.json`));
});
