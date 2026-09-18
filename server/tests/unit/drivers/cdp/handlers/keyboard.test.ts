/**
 * Unit tests for the keyboard handlers against a fake CDP connection: typing
 * at the focus or into an element, and named keys as key events.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { fakeDriver } from '../../../support/cdp.ts';

/** The key events sent, as [type, key]. */
const keys = (conn: any) => conn.sent('Input.dispatchKeyEvent').map((c: any) => [c.params.type, c.params.key]);

describe('typing', () => {
  it('types at the focus as inserted text', async () => {
    const { driver, conn } = fakeDriver();
    assert.deepEqual(await driver.dispatch('keyboard_type', { text: 'héllo' }), { ok: true });
    assert.deepEqual(conn.sent('Input.insertText')[0].params, { text: 'héllo' });
  });

  it('types into a named element, replacing its contents', async () => {
    const { driver, conn } = fakeDriver();
    conn.evaluate = (e) => (e.includes('scrollIntoView') ? { ok: true, data: { x: 1, y: 2 } } : undefined);
    await driver.dispatch('type', { element_id: 4, text: 'new' });
    assert.deepEqual(conn.methods(), ['Input.dispatchMouseEvent', 'Input.dispatchMouseEvent', 'Input.insertText']);
    const scripts = conn.sent('Runtime.evaluate').map((c) => c.params.expression);
    assert.ok(
      scripts.some((s) => s.includes('selectNodeContents')),
      'the old contents are selected first',
    );
  });

  it('sets a native date input to its value instead of typing into its segments', async () => {
    const { driver, conn } = fakeDriver();
    conn.evaluate = (e) =>
      e.includes('scrollIntoView')
        ? { ok: true, data: { x: 1, y: 2 } }
        : e.includes("el.tagName === 'INPUT'")
          ? 'date'
          : e.includes('el.value =')
            ? '2024-01-15'
            : undefined;
    assert.deepEqual(await driver.dispatch('type', { element_id: 4, text: 'Jan 15, 2024' }), {
      ok: true,
      value: '2024-01-15',
    });
    assert.equal(conn.sent('Input.insertText').length, 0, 'nothing is typed');
  });

  it('tells the agent the format when its text is not a date', async () => {
    const { driver, conn } = fakeDriver();
    conn.evaluate = (e) =>
      e.includes('scrollIntoView')
        ? { ok: true, data: { x: 1, y: 2 } }
        : e.includes("el.tagName === 'INPUT'")
          ? 'date'
          : undefined;
    assert.deepEqual(await driver.dispatch('type', { element_id: 4, text: 'soon' }), {
      ok: false,
      error: 'Could not read "soon" as a date value. Type it as YYYY-MM-DD.',
    });
  });

  it('reports a date the field refused', async () => {
    const { driver, conn } = fakeDriver();
    conn.evaluate = (e) =>
      e.includes('scrollIntoView')
        ? { ok: true, data: { x: 1, y: 2 } }
        : e.includes("el.tagName === 'INPUT'")
          ? 'date'
          : e.includes('el.value =')
            ? ''
            : undefined;
    assert.deepEqual(await driver.dispatch('type', { element_id: 4, text: '2024-01-15' }), {
      ok: false,
      error: 'The date field did not accept 2024-01-15',
    });
  });

  it('types at the focus when no element is named, and types empty text as empty', async () => {
    const { driver, conn } = fakeDriver();
    await driver.dispatch('type', {});
    assert.deepEqual(conn.methods(), ['Input.insertText']);
    assert.equal(conn.sent('Input.insertText')[0].params.text, '');
  });
});

describe('press-key', () => {
  it('presses Enter as keyDown, char and keyUp', async () => {
    const { driver, conn } = fakeDriver();
    assert.deepEqual(await driver.dispatch('press-key', { key: 'Enter' }), { ok: true });
    assert.deepEqual(keys(conn), [
      ['keyDown', 'Enter'],
      ['char', 'Enter'],
      ['keyUp', 'Enter'],
    ]);
    assert.equal(conn.sent('Input.dispatchKeyEvent')[0].params.windowsVirtualKeyCode, 13);
  });

  it('presses a key without text as keyDown and keyUp only', async () => {
    const { driver, conn } = fakeDriver();
    await driver.dispatch('press-key', { key: 'ArrowDown' });
    assert.deepEqual(keys(conn), [
      ['keyDown', 'ArrowDown'],
      ['keyUp', 'ArrowDown'],
    ]);
  });

  it('types a single printable character', async () => {
    const { driver, conn } = fakeDriver();
    assert.deepEqual(await driver.dispatch('press-key', { key: 'é' }), { ok: true });
    assert.deepEqual(conn.sent('Input.insertText')[0].params, { text: 'é' });
  });

  it('refuses a key it does not know', async () => {
    const { driver, conn } = fakeDriver();
    assert.deepEqual(await driver.dispatch('press-key', { key: 'Hyper' }), {
      ok: false,
      error: 'Unsupported key: Hyper',
    });
    assert.deepEqual(await driver.dispatch('press-key', {}), { ok: false, error: 'Unsupported key: undefined' });
    assert.equal(conn.calls.length, 0);
  });
});
