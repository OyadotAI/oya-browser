/**
 * The Claude plugin bundle lives in plugins/oya-browser, apart from the repo
 * root, because Anthropic's directory scans the whole plugin folder and refuses
 * any file over 5 MiB (the demo videos in assets/ are larger). The directory
 * also refuses, from an unverified publisher, a credential read from the
 * user's machine and download-and-run commands, so the bundle signs in with
 * OAuth and its skill copy has its own setup section. These tests keep the
 * bundle within those rules and in step with the root skill.
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

/** Where the part of the skill both copies share begins: the tool reference, after each copy's own setup. */
const SHARED_FROM = '## Over MCP';

/** The skill from the shared heading to the end. */
const sharedPart = (path) => {
  const text = read(path);
  return text.slice(text.indexOf(SHARED_FROM));
};

/** The bundle's copy of the skill. */
const PLUGIN_SKILL = `${BUNDLE}/skills/oya-browser/SKILL.md`;

test('the bundle skill shares the tool reference with skills/oya-browser, which npx skills installs', () => {
  assert.ok(read(PLUGIN_SKILL).includes(SHARED_FROM));
  assert.equal(sharedPart(PLUGIN_SKILL), sharedPart('skills/oya-browser/SKILL.md'));
});

test('the bundle skill has no download-and-run command', () => {
  assert.doesNotMatch(read(PLUGIN_SKILL), /\|\s*(sudo\s+)?(z?sh|bash|iex)\b|\birm\s+https?:|install-desktop\.|hdiutil/);
});

test('the bundle skill front matter is plain lines, with no JSON metadata', () => {
  const front = read(PLUGIN_SKILL).split('---')[1];
  assert.doesNotMatch(front, /[{}]/);
  assert.match(front, /^name: oya-browser$/m);
});

test('the bundle MCP server signs in with OAuth and reads nothing from the user machine', () => {
  const manifest = read(`${BUNDLE}/.claude-plugin/plugin.json`);
  assert.doesNotMatch(manifest, /\$\{/);
  assert.equal(JSON.parse(manifest).mcpServers.oya.headers, undefined);
});

test('the release script stamps the version into the bundle manifest', () => {
  assert.match(read('k8s/scripts/release.sh'), new RegExp(`${BUNDLE}/\\.claude-plugin/plugin\\.json`));
});
