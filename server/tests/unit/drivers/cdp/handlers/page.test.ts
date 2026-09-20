/**
 * Unit tests for the page handlers against a fake CDP connection:
 * screenshots, the analyzer, form values, waiting, scrolling to an edge,
 * cookies and main-world evaluation.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import {
  SCREENSHOT_QUALITY,
  WAIT_POLL_MS,
  WAIT_TIMEOUT_MS,
  MIN_STEP_MS,
} from '../../../../../src/drivers/cdp/constants.ts';
import { WORLD, fakeDriver } from '../../../support/cdp.ts';
import { advance } from '../../../support/http.ts';

describe('screenshot', () => {
  it('captures a JPEG of the viewport as a data URL', async () => {
    const { driver, conn } = fakeDriver();
    conn.replies['Page.captureScreenshot'] = { data: 'AAAA' };
    assert.deepEqual(await driver.dispatch('screenshot'), {
      ok: true,
      data: { screenshot: 'data:image/jpeg;base64,AAAA' },
    });
    assert.deepEqual(conn.sent('Page.captureScreenshot')[0].params, { format: 'jpeg', quality: SCREENSHOT_QUALITY });
  });

  it('honours the quality asked for', async () => {
    const { driver, conn } = fakeDriver();
    await driver.dispatch('screenshot', { quality: 90 });
    assert.equal(conn.sent('Page.captureScreenshot')[0].params.quality, 90);
  });
});

describe('analyze and read_page', () => {
  it('runs the analyzer in the isolated world with the caller’s options', async () => {
    const { driver, conn } = fakeDriver();
    conn.evaluate = (e) => (e.includes('analyzePage(') ? { ok: true, data: { elements: [] } } : undefined);
    assert.deepEqual(await driver.dispatch('analyze', { viewport_only: true }), { ok: true, data: { elements: [] } });
    const run = conn.sent('Runtime.evaluate').find((c) => c.params.expression.startsWith('(typeof analyzePage'))!;
    assert.equal(run.params.contextId, WORLD, 'never in the page’s own world');
    assert.ok(run.params.expression.includes('{"viewport_only":true}'));
  });

  it('lists the elements matching a selector, in the page and its iframes, under either name', async () => {
    const { driver, conn } = fakeDriver();
    const expected = { ok: true, data: { url: 'https://a.example/', title: 'A', elements: [{ tag: 'button' }] } };
    conn.evaluate = (e) => (e.includes('const describe = (el)') ? expected : undefined);
    assert.deepEqual(await driver.dispatch('read_page', { selector: 'button', limit: 5 }), expected);
    assert.deepEqual(await driver.dispatch('read_elements'), expected);
    const script = conn.sent('Runtime.evaluate').find((c) => c.params.expression.includes('const describe = (el)'))!;
    assert.ok(script.params.expression.includes(`("button", 5)`), 'the selector and limit reach the page');
    assert.ok(script.params.expression.includes("querySelectorAll('iframe, frame')"), 'iframes are searched');
  });

  it('reads blanks when the page cannot be read', async () => {
    const { driver, conn } = fakeDriver();
    conn.replies['Runtime.evaluate'] = new Error('boom');
    assert.deepEqual(await driver.dispatch('read_page'), { ok: true, data: { url: '', title: '', elements: [] } });
  });
});

describe('select', () => {
  it('sets the element’s value', async () => {
    const { driver, conn } = fakeDriver();
    conn.evaluate = (e) => e.includes('el.value =');
    assert.deepEqual(await driver.dispatch('select', { element_id: 5, value: 'b' }), { ok: true });
    const script = conn
      .sent('Runtime.evaluate')
      .map((c) => c.params.expression)
      .find((e) => e.includes('el.value ='))!;
    assert.ok(script.includes('"[data-ac-id=\\"5\\"]"') && script.includes('el.value = "b"'));
  });

  it('requires an element id, and says when the element is missing', async () => {
    const { driver } = fakeDriver();
    assert.deepEqual(await driver.dispatch('select', {}), { ok: false, error: 'element_id required' });
    assert.deepEqual(await driver.dispatch('select', { element_id: 5 }), { ok: false, error: 'Element not found' });
  });
});

describe('wait', () => {
  beforeEach(() => mock.timers.enable({ apis: ['setTimeout', 'Date'] }));
  afterEach(() => mock.timers.reset());

  it('answers as soon as the element is present', async () => {
    const { driver, conn } = fakeDriver();
    let checks = 0;
    conn.evaluate = (e) => (e.includes('querySelector(s)') ? ++checks >= 2 : undefined);
    const pending = driver.dispatch('wait', { selector: '#ready' });
    await advance(WAIT_POLL_MS);
    assert.deepEqual(await pending, { ok: true, data: { found: true } });
  });

  it('times out when the element never appears, capped by the command’s budget', async () => {
    const { driver } = fakeDriver();
    const pending = driver.dispatch('wait', { selector: '#never', timeout: WAIT_TIMEOUT_MS }, MIN_STEP_MS);
    await advance(WAIT_POLL_MS, MIN_STEP_MS / WAIT_POLL_MS);
    assert.deepEqual(await pending, { ok: false, error: 'Timeout' });
  });

  it('keeps waiting through a check that fails mid-navigation', async () => {
    const { driver, conn } = fakeDriver();
    let checks = 0;
    conn.replies['Runtime.evaluate'] = (p: any) =>
      p.expression.includes('querySelector(s)') && ++checks === 1
        ? new Error('boom')
        : { result: { value: checks > 1 } };
    const pending = driver.dispatch('wait', { selector: '#x' });
    await advance(WAIT_POLL_MS);
    assert.equal((await pending).ok, true);
  });
});

describe('scrolling to an edge', () => {
  it('scrolls to the top and to the bottom', async () => {
    const { driver, conn } = fakeDriver();
    await driver.dispatch('scroll-top');
    await driver.dispatch('scroll-bottom');
    const scripts = conn.sent('Runtime.evaluate').map((c) => c.params.expression);
    assert.ok(scripts.includes('window.scrollTo({ top: 0 })'));
    assert.ok(scripts.includes('window.scrollTo({ top: document.body.scrollHeight })'));
  });
});

describe('cookies and raw evaluation', () => {
  it('returns every cookie the browser holds', async () => {
    const { driver, conn } = fakeDriver();
    conn.replies['Network.getAllCookies'] = { cookies: [{ name: 'sid' }] };
    assert.deepEqual(await driver.dispatch('cookies'), { ok: true, data: { cookies: [{ name: 'sid' }] } });
  });

  it('evaluates server-internal scripts in the page’s main world', async () => {
    const { driver, conn } = fakeDriver();
    conn.evaluate = () => 42;
    assert.deepEqual(await driver.dispatch('evaluate_raw', { expression: '6*7' }), { ok: true, data: { result: 42 } });
    const run = conn.sent('Runtime.evaluate')[0].params;
    assert.deepEqual([run.expression, run.contextId], ['6*7', undefined]);
  });

  it('turns a page exception into an error', async () => {
    const { driver, conn } = fakeDriver();
    conn.replies['Runtime.evaluate'] = { exceptionDetails: { exception: { description: 'ReferenceError: x' } } };
    await assert.rejects(driver.dispatch('evaluate_raw', { expression: 'x' }), { message: 'ReferenceError: x' });
  });
});
