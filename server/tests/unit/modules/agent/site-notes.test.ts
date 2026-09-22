/**
 * Unit tests for the agent's memory of a site: a note is kept for the site the
 * page is on, shown the first time a later run reaches that site, and bounded.
 */
import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir();
const { rememberNote, notesHere } = await import('../../../../src/modules/agent/site-notes.ts');
const { noteAnalysis, forgetPage } = await import('../../../../src/modules/agent/changes.ts');
const { MAX_SITE_NOTES, MAX_NOTE_CHARS } = await import('../../../../src/modules/agent/constants.ts');
const keyConfig = await import('../../../../src/modules/config/service.ts');

const B = 'b-notes';

/** A run on the test browser for `apiKey`. */
const run = (apiKey = 'notes-key') => ({ apiKey, browserId: B });
/** Puts the browser on a page at `url`. */
const onPage = (url) => noteAnalysis(B, { facts: { url, title: 't' }, elements: [] });

describe('site notes', () => {
  beforeEach(() => {
    keyConfig.reset();
    forgetPage(B);
  });

  it('keeps a note for the site and shows it once to a later run that reaches it', async () => {
    onPage('https://shop.test/orders');
    assert.match(await rememberNote(run(), { note: 'Reports live under Account > Exports' }), /^Kept for shop.test/);
    const later = run();
    onPage('https://shop.test/');
    assert.match(notesHere(later), /NOTES YOU KEPT FOR shop.test[\s\S]*Reports live under Account > Exports/);
    assert.equal(notesHere(later), '', 'shown once per run');
  });

  it('keeps notes per key and per site', async () => {
    onPage('https://shop.test/');
    await rememberNote(run('key-a'), { note: 'a fact' });
    assert.equal(notesHere(run('key-b')), '');
    onPage('https://other.test/');
    assert.equal(notesHere(run('key-a')), '');
  });

  it('keeps only the newest notes, each cut to length', async () => {
    onPage('https://shop.test/');
    for (let i = 0; i <= MAX_SITE_NOTES; i++)
      await rememberNote(run(), { note: `note ${i} ${'x'.repeat(MAX_NOTE_CHARS)}` });
    const notes = keyConfig.getSiteNotes('notes-key', 'shop.test');
    assert.equal(notes.length, MAX_SITE_NOTES);
    assert.match(notes[0], /^note 1 /);
    assert.ok(notes.every((n) => n.length <= MAX_NOTE_CHARS));
  });

  it('refuses a note before any page, or an empty one', async () => {
    assert.match(await rememberNote(run(), { note: 'x' }), /^Error/);
    onPage('https://shop.test/');
    assert.match(await rememberNote(run(), { note: '  ' }), /^Error/);
  });
});
