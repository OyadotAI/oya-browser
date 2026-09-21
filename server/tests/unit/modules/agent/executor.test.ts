/**
 * Unit tests for running one tool for the model: unknown tools and thrown
 * errors come back as text, and a dialog raised by the action is appended.
 */
import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir();
const { executeTool } = await import('../../../../src/modules/agent/executor.ts');
const { scriptedBrowser } = await import('../../support/agent.ts');

const BROWSER = 'b-executor';
let browser;

describe('executeTool', () => {
  afterEach(() => browser?.disconnect());

  it('runs the named tool and returns its text', async () => {
    browser = scriptedBrowser(BROWSER);
    assert.equal(await executeTool(BROWSER, 'press_key', { key: 'Tab' }), 'Pressed Tab');
  });

  it('answers an unknown tool, including inherited names, without running anything', async () => {
    browser = scriptedBrowser(BROWSER);
    assert.equal(await executeTool(BROWSER, 'rm_rf', {}), 'Unknown tool: rm_rf');
    assert.equal(await executeTool(BROWSER, 'constructor', {}), 'Unknown tool: constructor');
    assert.equal(browser.calls.length, 0);
  });

  it('turns a thrown error into Error text', async () => {
    assert.equal(
      await executeTool('b-not-connected', 'press_key', { key: 'Tab' }),
      'Error: Browser b-not-connected not connected',
    );
  });

  it('appends a dialog that fired during the action, once', async () => {
    browser = scriptedBrowser(BROWSER, 'key-a', () => ({ ok: true, data: { dialog: 'Alert: "Card declined"' } }));
    assert.equal(await executeTool(BROWSER, 'click', { element_id: 1 }), 'Clicked element 1\n\nAlert: "Card declined"');
    browser.disconnect();
    browser = scriptedBrowser(BROWSER);
    assert.equal(await executeTool(BROWSER, 'click', { element_id: 1 }), 'Clicked element 1');
  });
});
