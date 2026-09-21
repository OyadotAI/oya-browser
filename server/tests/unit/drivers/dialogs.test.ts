/**
 * Unit tests for how native dialogs are described to the model, and which
 * kinds are answered automatically.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { AUTO_ACCEPT, describe as describeDialog } from '../../../src/drivers/dialogs.ts';

describe('dialogs', () => {
  it('answers only the one-button kinds automatically', () => {
    assert.deepEqual([...AUTO_ACCEPT].sort(), ['alert', 'beforeunload']);
  });

  it('reports an answered dialog with its text', () => {
    assert.equal(
      describeDialog({ type: 'alert', message: 'Saved' }, true),
      'Dialog (alert): "Saved", accepted automatically.',
    );
  });

  it('tells the model a held dialog blocks the page, with any default prompt', () => {
    assert.equal(
      describeDialog({ type: 'prompt', message: 'Name?', defaultPrompt: 'Ada' }),
      'A JavaScript prompt dialog is open: "Name?" (default: "Ada"). The page is blocked until you call handle_dialog.',
    );
    assert.match(
      describeDialog({ type: 'confirm', message: 'Delete?' }),
      /^A JavaScript confirm dialog is open: "Delete\?"\. /,
    );
  });
});
