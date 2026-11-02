/** Unit tests for src/shared/language.ts: a tag's primary language, and its name. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { languageName, primaryLanguage } from '../../../src/shared/language.ts';

it('reads the primary language of a tag, lower case, and nothing from nothing', () => {
  assert.equal(primaryLanguage('ja-JP'), 'ja');
  assert.equal(primaryLanguage('EN'), 'en');
  assert.equal(primaryLanguage(undefined), '');
});

it('names a language in the reader’s locale, and falls back to the tag itself', () => {
  assert.equal(languageName('ja-JP', 'en'), 'Japanese');
  assert.equal(languageName('de', 'de-DE'), 'Deutsch');
  assert.equal(languageName('ja', 'not a locale!'), 'ja');
});
