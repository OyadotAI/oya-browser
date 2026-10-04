/**
 * Unit tests for src/page/dialog-text.ts: which dialogs are answered for the
 * agent, and the sentence it reads about each.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { AUTO_ACCEPT, BLOCKED, describe as describeDialog } from '../../../src/page/dialog-text.ts';

describe('dialog text', () => {
  it('answers only the dialogs with one outcome', () => {
    assert.deepEqual([...AUTO_ACCEPT], ['alert', 'beforeunload']);
  });

  it('says an answered dialog was accepted', () => {
    assert.equal(
      describeDialog({ type: 'alert', message: 'Hi' }, true),
      'Dialog (alert): "Hi", accepted automatically.',
    );
  });

  it('says an open dialog blocks the page, with a prompt default when there is one', () => {
    const text = describeDialog({ type: 'prompt', message: 'Name?', defaultPrompt: 'Ann' });
    assert.equal(text, `A JavaScript prompt dialog is open: "Name?" (default: "Ann"). ${BLOCKED}`);
  });
});
