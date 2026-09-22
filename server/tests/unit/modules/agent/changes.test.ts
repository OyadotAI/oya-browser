/**
 * Unit tests for agent/changes.ts: what an action changed on the page, in words.
 */
import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { changeNote, noteAnalysis, forgetPage } from '../../../../src/modules/agent/changes.ts';

const B = 'b-changes';
/** An analysis with these facts and elements. */
const page = (facts: any, elements: any[] = []) => ({
  facts: { url: 'https://a.test/', title: 'A', elements: '5 total', ...facts },
  elements,
});

describe('changeNote', () => {
  beforeEach(() => forgetPage(B));

  it('says the page moved, and where to', () => {
    noteAnalysis(B, page({}));
    assert.match(
      changeNote(B, page({ url: 'https://a.test/done' })),
      /Changed: the page moved to https:\/\/a\.test\/done/,
    );
  });

  it('says a dialog opened, and names it', () => {
    noteAnalysis(B, page({}));
    assert.match(changeNote(B, page({ modal: 'Sign in (only this dialog was read)' })), /a dialog opened: Sign in/);
  });

  it('says a field now shows an error', () => {
    noteAnalysis(B, page({}, [{ text: 'Email', type: 'input' }]));
    const note = changeNote(B, page({}, [{ text: 'Email', type: 'input', error: 'Enter a valid email' }]));
    assert.match(note, /a field shows an error \(Email: Enter a valid email\)/);
  });

  it('says so when an action that should change the page did not', () => {
    noteAnalysis(B, page({}));
    assert.match(changeNote(B, page({}), true), /Nothing on the page changed/);
  });

  it('says nothing for an action not expected to change the page, like typing', () => {
    noteAnalysis(B, page({}));
    assert.equal(changeNote(B, page({})), '');
  });

  it('says nothing before it has seen the page once', () => {
    assert.equal(changeNote(B, page({ url: 'https://a.test/x' }), true), '');
  });
});
