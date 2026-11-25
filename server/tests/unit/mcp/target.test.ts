/**
 * Unit tests for where an MCP tool call goes: the pool's pick or a connector's
 * acquired browser, the note a fresh start adds, and the dialog cancelled for a
 * connector after a failed call.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { dismissDialog, noted, targetOf } from '../../../src/mcp/target.ts';
import { DIALOG_DISMISS_MS } from '../../../src/mcp/constants.ts';
import { disconnectBrowser } from '../support/fakes.ts';
import { driveBrowser, stubControl } from '../support/browsers.ts';

const B = 'b-target';

describe('targetOf', () => {
  it('is the picked browser, or the refusal when there is none', async () => {
    assert.deepEqual(await targetOf(false, { pick: () => B, noBrowser: 'none' }), { id: B });
    assert.deepEqual(await targetOf(false, { pick: () => null, noBrowser: 'none' }), { error: 'none' });
  });

  it('is a connector’s acquired browser instead of a pick', async () => {
    const acquire = async () => ({ id: 'own', note: 'Started.' });
    const found = await targetOf(true, { pick: () => B, acquire, noBrowser: 'none' });
    assert.deepEqual(found, { id: 'own', note: 'Started.' });
  });

  it('passes on whether the call opens a page, which is what may start a connector’s browser', async () => {
    const asked: boolean[] = [];
    const acquire = async (start: boolean) => (asked.push(start), { id: 'own' });
    await targetOf(true, { pick: () => null, acquire, noBrowser: 'none' });
    await targetOf(false, { pick: () => null, acquire, noBrowser: 'none' });
    assert.deepEqual(asked, [true, false]);
  });
});

describe('noted', () => {
  it('leads the reply with the note, when there is one', () => {
    assert.equal(noted('Started.', 'Navigated'), 'Started.\nNavigated');
    assert.equal(noted(undefined, 'Navigated'), 'Navigated');
  });
});

describe('dismissDialog', () => {
  afterEach(() => {
    mock.restoreAll();
    disconnectBrowser(B);
  });

  it('cancels a held dialog, never accepts it, and says so after the error', async () => {
    stubControl();
    const driver = driveBrowser(B, () => ({ ok: true, data: { accepted: false, type: 'confirm' } }));
    const said = await dismissDialog(B, 'The page asked "Delete?"');
    assert.deepEqual(driver.sent[0].params, { accept: false });
    assert.equal(driver.sent[0].timeout, DIALOG_DISMISS_MS, 'a wedged browser is not waited on twice');
    assert.match(said, /^The page asked "Delete\?"\nOya dismissed it with Cancel\./);
  });

  it('leaves the error as it was when no dialog is open', async () => {
    stubControl();
    driveBrowser(B, () => ({ ok: false, error: 'No dialog is open' }));
    assert.equal(await dismissDialog(B, 'Element not found'), 'Element not found');
  });

  it('leaves the error as it was when the browser cannot be asked', async () => {
    assert.equal(await dismissDialog('gone', 'Element not found'), 'Element not found');
  });
});
