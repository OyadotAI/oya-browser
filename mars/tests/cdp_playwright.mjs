// Drives Mars through Playwright's connectOverCDP, the way agent stacks do, and reports each step.
// Usage: node tests/cdp_playwright.mjs <cdp-port> <site-origin> <playwright-core dir> [puppeteer-core dir]
import { pathToFileURL } from 'node:url';
import { join } from 'node:path';

const [port, origin, playwrightDir, puppeteerDir] = process.argv.slice(2);
const results = [];
let failed = false;

/** Run one named step; a required step's failure fails the run. */
async function step(name, required, run) {
  try {
    const detail = await run();
    results.push({ step: name, ok: true, detail });
  } catch (error) {
    results.push({ step: name, ok: false, required, error: String(error && error.message || error).split('\n')[0] });
    if (required) failed = true;
  }
}

/** Deep equality for small JSON values. */
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const expect = (condition, message) => { if (!condition) throw new Error(message); };

async function playwright() {
  const { chromium } = await import(pathToFileURL(join(playwrightDir, 'index.mjs')).href);
  let browser;
  await step('playwright.connect', true, async () => {
    browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`, { timeout: 15000 });
    return browser.version();
  });
  if (!browser) return;
  const context = browser.contexts()[0];
  let page;
  await step('playwright.newPage', true, async () => { page = await context.newPage(); return 'ok'; });
  if (!page) return;
  page.setDefaultTimeout(8000);
  const consoleLines = [];
  page.on('console', message => consoleLines.push(message.text()));
  await step('playwright.goto', true, async () => {
    const response = await page.goto(`${origin}/`);
    return response ? response.status() : 'no response object';
  });
  await step('playwright.title', true, async () => { const title = await page.title(); expect(title === 'Mars CDP Lab', `title ${title}`); return title; });
  await step('playwright.evaluate.object', true, async () => {
    const data = await page.evaluate(() => window.data);
    expect(same(data, { answer: 42, list: [1, 2, 3], nested: { ok: true } }), JSON.stringify(data));
    return data;
  });
  await step('playwright.evaluate.async', true, async () => {
    const value = await page.evaluate(async () => { await new Promise(r => setTimeout(r, 5)); return 'later'; });
    expect(value === 'later', String(value)); return value;
  });
  await step('playwright.evaluate.handle', true, async () => {
    const handle = await page.evaluateHandle(() => document.body);
    const tag = await handle.evaluate(el => el.tagName);
    expect(tag === 'BODY', tag); return tag;
  });
  await step('playwright.$eval', true, async () => {
    const text = await page.$eval('#title', el => el.textContent);
    expect(text === 'Lab', text); return text;
  });
  await step('playwright.$$eval', true, async () => {
    const items = await page.$$eval('li', els => els.map(el => el.textContent));
    expect(same(items, ['one', 'two']), JSON.stringify(items)); return items;
  });
  await step('playwright.waitForSelector.attached', true, async () => {
    await page.waitForSelector('#late', { state: 'attached' }); return 'attached';
  });
  await step('playwright.waitForSelector.visible', false, async () => {
    await page.waitForSelector('#late', { state: 'visible', timeout: 3000 }); return 'visible';
  });
  await step('playwright.fill', true, async () => {
    await page.fill('#name', 'Ada', { force: true });
    const value = await page.inputValue('#name');
    expect(value === 'Ada', value); return value;
  });
  await step('playwright.fill.default', false, async () => {
    await page.fill('#name', 'Grace', { timeout: 3000 });
    return await page.inputValue('#name');
  });
  await step('playwright.click', true, async () => {
    await page.click('#go', { force: true, noWaitAfter: true });
    const greeting = await page.textContent('#greeting');
    expect(greeting.startsWith('Hello'), greeting); return greeting;
  });
  await step('playwright.click.default', false, async () => {
    await page.click('#go', { timeout: 3000, noWaitAfter: true }); return 'clicked';
  });
  await step('playwright.screenshot', true, async () => {
    const png = await page.screenshot();
    expect(png.length > 8 && png[0] === 0x89 && png[1] === 0x50, 'not a PNG'); return png.length;
  });
  await step('playwright.screenshot.element', false, async () => {
    const png = await (await page.$('#title')).screenshot();
    return png.length;
  });
  await step('playwright.console', true, async () => {
    await page.evaluate(() => console.log('hello from page'));
    expect(consoleLines.includes('hello from page'), JSON.stringify(consoleLines)); return consoleLines.length;
  });
  await step('playwright.route.fulfill', true, async () => {
    await page.route('**/done', route => route.fulfill({ status: 200, contentType: 'text/html', body: '<title>Routed</title><h1>Routed</h1>' }));
    await page.goto(`${origin}/done`);
    const title = await page.title();
    await page.unroute('**/done');
    expect(title === 'Routed', title); return title;
  });
  await step('playwright.route.abort', true, async () => {
    await page.route('**/missing', route => route.abort('failed'));
    let message = '';
    try { await page.goto(`${origin}/missing`); } catch (error) { message = String(error.message); }
    await page.unroute('**/missing');
    expect(message.includes('ERR_FAILED'), message); return 'aborted';
  });
  await step('playwright.cookies', true, async () => {
    await context.addCookies([{ name: 'sid', value: 'one', url: origin }]);
    const cookies = await context.cookies(origin);
    expect(cookies.some(cookie => cookie.name === 'sid' && cookie.value === 'one'), JSON.stringify(cookies));
    await page.goto(`${origin}/`);
    const seen = await page.evaluate(() => document.cookie);
    expect(seen.includes('sid=one'), seen);
    await context.clearCookies();
    const after = await context.cookies(origin);
    expect(after.length === 0, JSON.stringify(after)); return 'cookies ok';
  });
  await step('playwright.navigation.link', true, async () => {
    await Promise.all([page.waitForNavigation(), page.click('#link', { force: true })]);
    expect(page.url().endsWith('/done'), page.url()); return page.url();
  });
  await step('playwright.goBack', true, async () => {
    await page.goBack();
    expect(page.url() === `${origin}/`, page.url()); return page.url();
  });
  await step('playwright.ownership', true, async () => {
    // The person takes the tab back while this page loads; the load itself may already be refused.
    const cdp = await context.newCDPSession(page);
    try { await page.goto(`${origin}/handoff`); } catch {}
    let message = '';
    try { await cdp.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x: 10, y: 10, button: 'left', clickCount: 1 }); } catch (error) { message = String(error.message); }
    expect(message.includes('control_denied'), message);
    let evaluated = '';
    try { await cdp.send('Runtime.evaluate', { expression: 'document.title = "taken"', returnByValue: true }); evaluated = 'committed?'; } catch (error) { evaluated = String(error.message); }
    const title = await page.title().catch(error => String(error.message));
    expect(title !== 'taken', `peek must not commit: ${evaluated} ${title}`);
    return 'refused after takeover';
  });
  await step('playwright.close', true, async () => { await browser.close(); return 'closed'; });
}

async function puppeteer() {
  if (!puppeteerDir) return;
  const puppeteer = (await import(pathToFileURL(join(puppeteerDir, 'lib/puppeteer/puppeteer-core.js')).href)).default;
  let browser;
  await step('puppeteer.connect', false, async () => {
    browser = await puppeteer.connect({ browserURL: `http://127.0.0.1:${port}`, protocolTimeout: 15000 });
    return await browser.version();
  });
  if (!browser) return;
  let page;
  await step('puppeteer.newPage', false, async () => { page = await browser.newPage(); return 'ok'; });
  if (!page) return;
  page.setDefaultTimeout(8000);
  await step('puppeteer.goto', false, async () => { const response = await page.goto(`${origin}/`); return response ? response.status() : 'no response'; });
  await step('puppeteer.title', false, async () => { const title = await page.title(); expect(title === 'Mars CDP Lab', title); return title; });
  await step('puppeteer.evaluate', false, async () => { const data = await page.evaluate(() => window.data); expect(data.answer === 42, JSON.stringify(data)); return data; });
  await step('puppeteer.$eval', false, async () => { const text = await page.$eval('#title', el => el.textContent); expect(text === 'Lab', text); return text; });
  await step('puppeteer.type', false, async () => { await page.type('#name', 'Ada'); const value = await page.$eval('#name', el => el.value); expect(value === 'Ada', value); return value; });
  await step('puppeteer.click', false, async () => { await page.click('#go'); const greeting = await page.$eval('#greeting', el => el.textContent); expect(greeting.startsWith('Hello'), greeting); return greeting; });
  await step('puppeteer.screenshot', false, async () => { const png = await page.screenshot(); return png.length; });
  await step('puppeteer.cookies', false, async () => { await browser.setCookie({ name: 'pp', value: '1', url: origin }); const cookies = await browser.cookies(); expect(cookies.some(c => c.name === 'pp'), JSON.stringify(cookies)); return cookies.length; });
  await step('puppeteer.close', false, async () => { await browser.disconnect(); return 'closed'; });
}

try {
  await playwright();
  await puppeteer();
} catch (error) {
  results.push({ step: 'run', ok: false, required: true, error: String(error && error.stack || error) });
  failed = true;
}
for (const result of results) console.log(JSON.stringify(result));
process.exit(failed ? 1 : 0);
