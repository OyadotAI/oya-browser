/**
 * Unit tests for the CDPDriver surface against a fake connection: native
 * dialogs answered or held around a command, the isolated world rebuilt after
 * a navigation, and the small helpers other code relies on.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { FALLBACK_VIEWPORT } from '../../../../src/drivers/cdp/constants.ts';
import { SESSION, WORLD, fakeDriver } from '../../support/cdp.ts';

describe('CDPDriver.send and dialogs', () => {
  it('accepts an alert on its own and reports it with the command’s result', async () => {
    const { driver, conn } = fakeDriver();
    conn.replies['Page.reload'] = () => {
      conn.emit('Page.javascriptDialogOpening', { type: 'alert', message: 'Saved' });
      return {};
    };
    const result = await driver.send('reload');
    assert.deepEqual(result, { ok: true, data: { dialog: 'Dialog (alert): "Saved", accepted automatically.' } });
    assert.deepEqual(conn.sent('Page.handleJavaScriptDialog')[0].params, { accept: true });
  });

  it('answers at once when a confirm is held open mid-command', async () => {
    const { driver, conn } = fakeDriver();
    conn.replies['Page.reload'] = () => {
      // The event arrives after the command went out, while the renderer is blocked.
      setImmediate(() => conn.emit('Page.javascriptDialogOpening', { type: 'confirm', message: 'Leave?' }));
      return new Promise(() => {});
    };
    const result = await driver.send('reload');
    assert.equal(result.ok, false);
    assert.match(result.error, /confirm dialog is open: "Leave\?"/);
    assert.equal(conn.sent('Page.handleJavaScriptDialog').length, 0, 'a decision is left to the caller');
  });

  it('refuses other commands while a dialog is held, then answers it', async () => {
    const { driver, conn } = fakeDriver();
    conn.emit('Page.javascriptDialogOpening', { type: 'prompt', message: 'Name?', defaultPrompt: '' });
    assert.match((await driver.send('screenshot')).error, /call handle_dialog/);
    const answered = await driver.send('handle_dialog', { accept: true, prompt_text: 'Ada' });
    assert.deepEqual(answered, { ok: true, data: { type: 'prompt', message: 'Name?', accepted: true } });
    assert.deepEqual(conn.sent('Page.handleJavaScriptDialog')[0], {
      method: 'Page.handleJavaScriptDialog',
      params: { accept: true, promptText: 'Ada' },
      sessionId: SESSION,
    });
    conn.replies['Page.captureScreenshot'] = { data: 'x' };
    assert.equal((await driver.send('screenshot')).ok, true);
  });

  it('dismisses a held confirm and sends no prompt text for it', async () => {
    const { driver, conn } = fakeDriver();
    conn.emit('Page.javascriptDialogOpening', { type: 'confirm', message: 'Delete?' });
    await driver.send('handle_dialog', { accept: false, promptText: 'ignored' });
    assert.deepEqual(conn.sent('Page.handleJavaScriptDialog')[0].params, { accept: false });
  });

  it('says so when asked to answer a dialog that is not open', async () => {
    const { driver } = fakeDriver();
    assert.deepEqual(await driver.send('handle_dialog', {}), { ok: false, error: 'No dialog is open' });
  });

  it('ignores dialogs in other sessions and forgets one the page closed', async () => {
    const { driver, conn } = fakeDriver();
    conn.emit('Page.javascriptDialogOpening', { type: 'confirm', message: 'x' }, 'other-session');
    assert.equal(driver.pendingDialog, undefined);
    conn.emit('Page.javascriptDialogOpening', { type: 'confirm', message: 'x' });
    conn.emit('Page.javascriptDialogClosed', {});
    assert.equal(driver.pendingDialog, null);
  });

  it('registers its dialog listeners only once', () => {
    const { driver, conn } = fakeDriver();
    driver.watchDialogs();
    assert.equal(conn.listeners.get('Page.javascriptDialogOpening')!.size, 1);
  });

  it('passes a command failure through', async () => {
    const { driver, conn } = fakeDriver();
    conn.replies['Page.reload'] = new Error('target closed');
    await assert.rejects(driver.send('reload'), /target closed/);
  });
});

describe('CDPDriver.evaluate', () => {
  it('creates the isolated world once, with the analyzer in it', async () => {
    const { driver, conn } = fakeDriver();
    conn.evaluate = (e) => (e === '1+1' ? 2 : undefined);
    assert.equal(await driver.evaluate('1+1'), 2);
    assert.equal(await driver.evaluate('1+1'), 2);
    assert.equal(conn.sent('Page.createIsolatedWorld').length, 1);
    const [load] = conn.sent('Runtime.evaluate');
    assert.equal(load.params.contextId, WORLD);
    assert.ok(load.params.expression.includes('data-test'), 'the analyzer carries the session’s tag attribute');
  });

  it('rebuilds the world once after a navigation destroyed it', async () => {
    const { driver, conn } = fakeDriver();
    let fails = 1;
    conn.replies['Runtime.evaluate'] = (p: any) =>
      p.expression === 'x' && fails-- > 0
        ? new Error('Cannot find context with specified id')
        : { result: { value: 'ok' } };
    assert.equal(await driver.evaluate('x'), 'ok');
    assert.equal(conn.sent('Page.createIsolatedWorld').length, 2);
  });

  it('does not retry an ordinary failure', async () => {
    const { driver, conn } = fakeDriver();
    conn.replies['Runtime.evaluate'] = (p: any) => (p.expression === 'x' ? new Error('boom') : { result: {} });
    await assert.rejects(driver.evaluate('x'), /boom/);
    assert.equal(conn.sent('Page.createIsolatedWorld').length, 1);
  });
});

describe('CDPDriver helpers', () => {
  it('is alive until its connection closes', () => {
    const { driver, conn } = fakeDriver();
    assert.equal(driver.isAlive(), true);
    driver.close();
    assert.equal(conn.closed, true);
    assert.equal(driver.isAlive(), false);
  });

  it('reports the viewport, falling back to 1280×800', async () => {
    const { driver, conn } = fakeDriver();
    assert.deepEqual(await driver.viewport(), FALLBACK_VIEWPORT);
    conn.evaluate = (e) => (e.includes('innerWidth') ? { width: 10, height: 20 } : undefined);
    assert.deepEqual(await driver.viewport(), { width: 10, height: 20 });
  });

  it('is a CDP client that advertises its capabilities', () => {
    const { driver } = fakeDriver();
    assert.equal(driver.clientType, 'cdp');
    assert.ok(driver.capabilities.has('navigate'));
  });

  it('takes Accept-Language from the persona’s languages', () => {
    const { driver } = fakeDriver({ fingerprint: { navigator: { languages: ['de-DE', 'de'] } } });
    assert.equal(driver.acceptLanguage, 'de-DE,de');
  });
});
