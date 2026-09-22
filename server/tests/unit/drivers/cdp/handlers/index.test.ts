/**
 * Unit tests for the CDP driver's command map: both spellings of the action
 * vocabulary reach the same handler, an unsupported action is answered rather
 * than thrown, and a closed browser is refused.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { HANDLERS, dispatch } from '../../../../../src/drivers/cdp/handlers/index.ts';
import { CDP_CAPABILITIES } from '../../../../../src/drivers/cdp/actions.ts';
import { MIN_STEP_MS } from '../../../../../src/drivers/cdp/constants.ts';
import { fakeDriver } from '../../../support/cdp.ts';

describe('dispatch', () => {
  it('answers an unsupported action instead of throwing', async () => {
    const { driver, conn } = fakeDriver();
    assert.deepEqual(await dispatch(driver, 'teleport', {}, 1000), {
      ok: false,
      error: 'Unsupported action for a CDP browser: teleport',
    });
    assert.equal(conn.calls.length, 0);
  });

  it('never dispatches to an inherited property', async () => {
    const { driver } = fakeDriver();
    const result = await dispatch(driver, 'constructor', {}, 1000);
    assert.equal(result.ok, false);
    assert.match(result.error, /^Unsupported action for a CDP browser/);
  });

  it('refuses an action that is not a string, so a wrapped server-internal name cannot reach its handler', async () => {
    const { driver, conn } = fakeDriver();
    for (const action of [['evaluate_raw'], { a: 1 }, 7]) {
      assert.deepEqual(await dispatch(driver, action, { expression: '1' }, 1000), {
        ok: false,
        error: 'action must be a string',
      });
    }
    assert.equal(conn.calls.length, 0);
  });

  it('refuses a browser whose connection has closed', async () => {
    const { driver, conn } = fakeDriver();
    conn.close();
    await assert.rejects(dispatch(driver, 'reload', {}, 1000), { message: 'Browser not connected' });
  });

  it('accepts the Oya client’s underscored spellings', async () => {
    const { driver, conn } = fakeDriver();
    await dispatch(driver, 'press_key', { key: 'Tab' }, 1000);
    await dispatch(driver, 'click_coordinates', { x: 1, y: 2 }, 1000);
    conn.replies['Target.getTargets'] = { targetInfos: [] };
    assert.deepEqual(await dispatch(driver, 'list_tabs', {}, 1000), { ok: true, data: { tabs: [] } });
    assert.deepEqual(conn.methods().slice(0, 5), [
      'Input.dispatchKeyEvent',
      'Input.dispatchKeyEvent',
      'Input.dispatchMouseEvent',
      'Input.dispatchMouseEvent',
      'Target.getTargets',
    ]);
  });

  it('reads the scroll direction out of the params', async () => {
    const { driver, conn } = fakeDriver();
    conn.evaluate = () => ({ width: 1000, height: 500 });
    await dispatch(driver, 'scroll', { direction: 'up', amount: 50 }, 1000);
    assert.equal(conn.sent('Input.dispatchMouseEvent')[0].params.deltaY, -50);
  });

  it('never gives a step less than the minimum, however little budget is left', async () => {
    const { driver, conn } = fakeDriver();
    const seen: number[] = [];
    conn.send = async (method: string, _params: any, _sid?: string, timeoutMs?: number) => {
      if (method === 'Page.reload') seen.push(timeoutMs!);
      return {};
    };
    await dispatch(driver, 'reload', {}, 0);
    assert.equal(seen[0], MIN_STEP_MS);
  });

  it('has a handler for every advertised capability except the dialog answer', async () => {
    const { driver } = fakeDriver();
    for (const action of CDP_CAPABILITIES) {
      if (action === 'handle_dialog') continue;
      const { action: mapped } = (await import('../../../../../src/drivers/cdp/actions.ts')).normalise(action, {});
      assert.ok(Object.hasOwn(HANDLERS, mapped) || action === 'scroll', `${action} → ${mapped}`);
    }
    assert.ok(driver);
  });
});
