/**
 * Unit tests for Browser (src/browser.ts): commands, element ids, CAPTCHA
 * after navigation, agent answers, and the links it hands out.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { OyaError } from '../../dist/index.js';
import { BASE, client, type Route } from './support/fake-fetch.ts';

const CMD = 'POST /api/browsers/b1/command';

/** A reattached browser `b1` on a fake fetch, with `routes` added. */
async function browser(routes: Record<string, Route | Route[]> = {}) {
  const { oya, calls } = client({
    'GET /api/browsers/b1': { body: { id: 'b1', provider: 'steel', persona: 'p1', cdpUrl: 'wss://cdp' } },
    ...routes,
  });
  const b = await oya.browser.get('b1');
  calls.length = 0;
  return { b, calls };
}

describe('Browser commands', () => {
  it('sends the action and params, and returns the command data', async () => {
    const { b, calls } = await browser({ [CMD]: { body: { ok: true, data: { markdown: '# hi', elements: [] } } } });
    assert.equal((await b.analyze()).markdown, '# hi');
    assert.deepEqual(calls[0].body, { action: 'analyze', params: {} });
  });

  it('asks for the page in the format given, and returns it with its blocks', async () => {
    const data = {
      format: 'toon',
      page: 'page:\n  url: x',
      blocks: [{ region: 'main', kind: 'h1', text: 'Hi' }],
      elements: [],
    };
    const { b, calls } = await browser({ [CMD]: { body: { ok: true, data } } });
    const analysis = await b.analyze({ format: 'toon' });
    assert.equal(analysis.page, 'page:\n  url: x');
    assert.equal(analysis.blocks?.[0].kind, 'h1');
    assert.deepEqual(calls[0].body, { action: 'analyze', params: { format: 'toon' } });
  });

  it('throws a 422 OyaError when a command ran and failed', async () => {
    const { b } = await browser({ [CMD]: { body: { ok: false, error: 'no such element' } } });
    await assert.rejects(b.pressKey('Enter'), { name: 'OyaError', status: 422, message: 'no such element' });
  });

  it('names the action when a failed command gave no reason', async () => {
    const { b } = await browser({ [CMD]: { body: { ok: false } } });
    await assert.rejects(b.pressKey('Enter'), { message: 'press_key failed' });
  });

  it('keeps only visible elements', async () => {
    const elements = [
      { id: 1, visible: true },
      { id: 2, visible: false },
    ];
    const { b } = await browser({ [CMD]: { body: { ok: true, data: { markdown: '', elements } } } });
    assert.deepEqual(
      (await b.elements()).map((e) => e.id),
      [1],
    );
  });

  it('reads tabs, the active URL and a new tab id', async () => {
    const tabs = [
      { id: 't1', url: 'https://a', title: 'A', active: false },
      { id: 't2', url: 'https://b', title: 'B', active: true },
    ];
    const { b } = await browser({
      [CMD]: [
        { body: { ok: true, data: { tabs } } },
        { body: { ok: true, data: {} } },
        { body: { ok: true, data: { tab_id: 't3' } } },
      ],
    });
    assert.equal(await b.url(), 'https://b');
    assert.deepEqual(await b.tabs(), [], 'a missing tab list reads as empty');
    assert.equal(await b.openTab('https://c'), 't3');
  });
});

describe('Browser element ids', () => {
  it('accepts a number or numeric string and targets it by data-ac-id', async () => {
    const { b, calls } = await browser({ [CMD]: { body: { ok: true } } });
    await b.click('7');
    await b.type(8, 'hello');
    assert.deepEqual(calls[0].body, { action: 'click', params: { element_id: 7, selector: '[data-ac-id="7"]' } });
    assert.deepEqual(calls[1].body, {
      action: 'type',
      params: { element_id: 8, selector: '[data-ac-id="8"]', text: 'hello' },
    });
  });

  for (const bad of ['', 'abc', -1, 1.5, '#submit']) {
    it(`refuses ${JSON.stringify(bad)} with a 400 before calling the server`, async () => {
      const { b, calls } = await browser();
      await assert.rejects(b.click(bad as number), (e: unknown) => e instanceof OyaError && e.status === 400);
      assert.equal(calls.length, 0);
    });
  }
});

describe('Browser.scroll', () => {
  it('sends a plain scroll as direction and amount', async () => {
    const { b, calls } = await browser({ [CMD]: { body: { ok: true } } });
    await b.scroll('down');
    assert.deepEqual(calls[0].body, { action: 'scroll', params: { direction: 'down' } });
  });

  it('aims a scroll at a point as one unsmoothed wheel event of 500px by default', async () => {
    const { b, calls } = await browser({ [CMD]: { body: { ok: true } } });
    await b.scroll('up', undefined, { x: 10, y: 20 });
    assert.deepEqual(calls[0].body, {
      action: 'scroll',
      params: { direction: 'up', amount: 500, x: 10, y: 20, smooth: false },
    });
  });
});

describe('Browser.goto with captcha: auto', () => {
  /** A started browser with automatic CAPTCHA solving. */
  async function autoBrowser(captcha: Route) {
    const { oya, calls } = client({
      'POST /api/browsers/start': { body: { id: 'b1', provider: 'oya-cloud', persona: 'p', status: 'ready' } },
      [CMD]: { body: { ok: true } },
      'POST /api/browsers/b1/captcha': captcha,
    });
    return { b: await oya.browser.start({ captcha: 'auto' }), calls };
  }

  it('solves after navigating and passes when cleared or invisible', async () => {
    const { b, calls } = await autoBrowser({ body: { present: true, solved: false, invisible: true } });
    await b.goto('https://x');
    assert.deepEqual(
      calls.slice(1).map((c) => c.path),
      ['/api/browsers/b1/command', '/api/browsers/b1/captcha'],
    );
  });

  it('throws a 409 when a CAPTCHA is still on screen', async () => {
    const { b } = await autoBrowser({ body: { present: true, solved: false, error: 'solver out of credit' } });
    await assert.rejects(b.goto('https://x'), { status: 409, message: 'solver out of credit' });
  });

  it('does not solve when auto CAPTCHA is off', async () => {
    const { b, calls } = await browser({ [CMD]: { body: { ok: true } } });
    await b.goto('https://x');
    assert.equal(calls.length, 1);
  });
});

describe('Browser agent calls', () => {
  it('ask sends the prompt with data and secrets, and returns the text', async () => {
    const { b, calls } = await browser({ 'POST /api/browsers/b1/chat': { body: { text: 'done' } } });
    assert.equal(await b.ask('go {{x}}', { data: { x: 1 }, secrets: { pw: 's' } }), 'done');
    assert.deepEqual(calls[0].body, {
      messages: [{ role: 'user', content: 'go {{x}}' }],
      data: { x: 1 },
      secrets: { pw: 's' },
    });
  });

  it('ask throws a failure the server put in a 200 body', async () => {
    const { b } = await browser({ 'POST /api/browsers/b1/chat': { body: { error: 'quota', status: 429 } } });
    await assert.rejects(b.ask('go'), { status: 429, message: 'quota' });
  });

  it('extract sends the schema and returns the data in its shape', async () => {
    const schema = { type: 'object', properties: { price: { type: 'string' } } };
    const { b, calls } = await browser({
      'POST /api/browsers/b1/chat': { body: { text: 'DONE: {}', data: { price: '$5' } } },
    });
    assert.deepEqual(await b.extract('price?', schema, { data: { q: 'x' } }), { price: '$5' });
    assert.deepEqual(calls[0].body, { messages: [{ role: 'user', content: 'price?' }], data: { q: 'x' }, schema });
  });

  it('extract throws with the report when the agent could not get the data', async () => {
    const { b } = await browser({ 'POST /api/browsers/b1/chat': { body: { text: 'FAILED: no price', failed: true } } });
    await assert.rejects(b.extract('price?', {}), { status: 422, message: 'FAILED: no price' });
  });

  it('play sends variables with autoHeal on by default, and defaults a failure to 500', async () => {
    const { b, calls } = await browser({
      'POST /api/browsers/b1/playbooks/my%20flow/play': [
        { body: { steps: 3, total: 3, fellBack: false } },
        { body: { error: 'broken' } },
      ],
    });
    assert.equal((await b.play('my flow', { a: 'b' })).steps, 3);
    assert.deepEqual(calls[0].body, { variables: { a: 'b' }, autoHeal: true });
    await assert.rejects(b.play('my flow', {}, { autoHeal: false }), { status: 500, message: 'broken' });
  });

  it('completeMfa makes a relative live-view link absolute', async () => {
    const { b } = await browser({
      'POST /api/browsers/b1/mfa': { body: { present: true, completed: false, liveViewUrl: '/dashboard/?browser=b1' } },
    });
    assert.equal((await b.completeMfa()).liveViewUrl, `${BASE}/dashboard/?browser=b1`);
  });
});

describe('Browser links', () => {
  it('liveViewUrl is the console deep link, with no credential in it', async () => {
    const { b, calls } = await browser();
    assert.equal(b.liveViewUrl(), `${BASE}/dashboard/?browser=b1`);
    assert.equal(calls.length, 0);
  });

  it('liveStreamUrl mints a ticket and puts it in the query', async () => {
    const { b } = await browser({ 'POST /api/control/sessions/b1/ticket': { body: { ticket: 't/1' } } });
    assert.equal(await b.liveStreamUrl(), `${BASE}/api/live/b1?ticket=t%2F1`);
  });

  it('shareUrl puts the token in the fragment and defaults to view-only for an hour', async () => {
    const { b, calls } = await browser({
      'POST /api/control/sessions/b1/share': { body: { id: 'c1', token: 'tok', expiresAt: 9 } },
    });
    assert.deepEqual(await b.shareUrl(), { url: `${BASE}/live/b1#t=tok`, id: 'c1', expiresAt: 9 });
    assert.deepEqual(calls[0].body, { control: false, expiresIn: 3600 });
  });

  it('exposes the CDP URL it was given', async () => {
    const { b } = await browser();
    assert.deepEqual([b.id, b.provider, b.persona, b.cdpUrl], ['b1', 'steel', 'p1', 'wss://cdp']);
  });
});

describe('Browser.stop', () => {
  it('stop, close and await-using all stop the browser', async () => {
    const { b, calls } = await browser({ 'POST /api/browsers/b1/stop': { body: { id: 'b1', ok: true } } });
    await b.stop();
    await b.close();
    await b[Symbol.asyncDispose]();
    assert.deepEqual(
      calls.map((c) => c.path),
      Array(3).fill('/api/browsers/b1/stop'),
    );
  });
});
