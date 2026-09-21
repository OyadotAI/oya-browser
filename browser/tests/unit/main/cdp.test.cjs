/**
 * Unit tests for main/cdp.cjs: commands go to the view's own debugger,
 * attached on first use, and never to a destroyed view.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { FakeView } = require('../support/fakes.cjs');
const { cdpAttach, cdp, cdpEval } = require('../../../main/cdp.cjs');

describe('cdp', () => {
  it('attaches the debugger on first use and reuses it', () => {
    const view = new FakeView();
    assert.equal(cdpAttach(view), view.webContents.debugger);
    assert.equal(view.webContents.debugger.attached, true);
  });

  it('refuses a missing or destroyed view', async () => {
    const view = new FakeView();
    view.webContents.destroy();
    assert.equal(cdpAttach(view), null);
    assert.equal(cdpAttach(null), null);
    await assert.rejects(cdp(view, 'Page.enable'), /View is destroyed/);
  });

  it('sends a command with its params', async () => {
    const view = new FakeView({ debuggerResponses: { 'Page.navigate': { frameId: 'f' } } });
    assert.deepEqual(await cdp(view, 'Page.navigate', { url: 'x' }), { frameId: 'f' });
    assert.deepEqual(view.webContents.debugger.sent[0].params, { url: 'x' });
  });

  it('returns an evaluation by value', async () => {
    const view = new FakeView({ debuggerResponses: { 'Runtime.evaluate': { result: { value: 7 } } } });
    assert.equal(await cdpEval(view, '3 + 4'), 7);
    assert.deepEqual(view.webContents.debugger.sent[0].params, {
      expression: '3 + 4',
      returnByValue: true,
      awaitPromise: true,
    });
  });

  it('throws the page exception of a failed evaluation', async () => {
    const view = new FakeView({ debuggerResponses: { 'Runtime.evaluate': { exceptionDetails: { text: 'boom' } } } });
    await assert.rejects(cdpEval(view, 'x'), /boom/);
  });
});
