/** Real user input -> CDP recording -> exported Playwright -> fresh-page replay.
 * Requires the UI's Playwright dev dependency and a local Chrome installation.
 */
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdtempSync, existsSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { chromium } from '../ui/node_modules/playwright/index.mjs';
import { CDPDriver } from './src/drivers/cdp.js';

const executable = ['/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  '/usr/bin/google-chrome', '/usr/bin/chromium', '/usr/bin/chromium-browser'].find(existsSync);
assert(executable, 'Chrome is required for recording/export acceptance tests');
const scratch = mkdtempSync(join(tmpdir(), 'oya-recording-replay-'));
process.env.OYA_DATA_DIR = join(scratch, 'data');
const { sanitizeSteps, templateValues, renderPlaywright, variablesOf } = await import('./src/playbook.js');
const fixture = `<!doctype html><title>Recording acceptance</title>
  <form action="/done">
    <label>Member<input id="member" name="member"></label>
    <label>Password<input id="password" name="password" type="password"></label>
    <label>Notes<textarea id="notes" name="notes"></textarea></label>
    <label>Clear<input id="clear" name="clear" value="remove me"></label>
    <div id="editor" contenteditable="true" aria-label="Description"></div>
    <label>Plan<select id="plan" name="plan" size="2"><option selected>Basic</option><option>Plus</option></select></label>
    <label id="agreeLabel" for="agree">Agree</label><input id="agree" name="agree" type="checkbox" style="display:none">
    <input type="hidden" id="backing" name="backing">
    <div style="display:none"><input id="noise"></div>
    <button type="button" id="choose" onclick="backing.value='chosen'; noise.value='noise'; noise.dispatchEvent(new Event('input',{bubbles:true})); backing.dispatchEvent(new Event('change',{bubbles:true}))">Choose</button>
    <button id="submit">Continue</button>
  </form>`;
const site = createServer((req, res) => {
  res.setHeader('Content-Type', 'text/html');
  res.end(req.url.startsWith('/done') ? '<!doctype html><title>Completed</title><p>Completed</p>' : fixture);
});
let chrome, browser, driver;
try {
  await new Promise(resolve => site.listen(0, '127.0.0.1', resolve));
  const url = `http://127.0.0.1:${site.address().port}/`;
  chrome = spawn(executable, ['--headless=new', '--remote-debugging-port=0', '--no-first-run',
    '--no-default-browser-check', `--user-data-dir=${join(scratch, 'chrome')}`, 'about:blank'],
  { stdio: ['ignore', 'ignore', 'pipe'] });
  const endpoint = await new Promise((resolve, reject) => {
    let output = '';
    const timer = setTimeout(() => reject(new Error('Chrome startup timed out')), 20000);
    chrome.stderr.on('data', chunk => {
      output += chunk;
      const match = output.match(/ws:\/\/127\.0\.0\.1:\d+\/devtools\/browser\/[a-f0-9-]+/);
      if (match) { clearTimeout(timer); resolve(match[0]); }
    });
    chrome.once('error', error => { clearTimeout(timer); reject(error); });
    chrome.once('exit', () => { clearTimeout(timer); reject(new Error('Chrome exited')); });
  });
  browser = await chromium.connectOverCDP(endpoint);
  const page = browser.contexts()[0].pages()[0];
  page.setDefaultTimeout(5000);
  await page.goto(url);
  driver = await new CDPDriver({ wsUrl: endpoint, provider: 'chrome' }).connect();
  assert((await driver.send('record', { mode: 'start' })).ok);
  const member = 'member \' " ` ${literal} \\';
  await page.locator('#member').fill(member);
  await page.locator('#password').fill('recorded-secret');
  await page.locator('#notes').fill('First line\nSecond line');
  await page.locator('#clear').fill('');
  await page.locator('#editor').fill('Editable description');
  await page.locator('#plan').focus();
  await page.keyboard.press('ArrowDown');
  await page.locator('#agreeLabel').click();
  await page.locator('#choose').click();
  await page.locator('#submit').click();
  await page.waitForURL('**/done?**');
  const recording = await driver.send('record', { mode: 'stop' });
  assert(recording.ok);
  assert(!JSON.stringify(recording).includes('recorded-secret'));
  assert(!recording.data.steps.some(step => ['noise', 'backing', 'agree'].includes(step.el?.domId)));
  assert.equal(recording.data.steps.filter(step => step.el?.domId === 'agreeLabel').length, 1);
  const pb = templateValues({ name: 'acceptance', defaults: {},
    secrets: recording.data.secrets, steps: sanitizeSteps(recording.data.steps) });
  assert.deepEqual(variablesOf(pb.steps), ['member', 'password', 'notes', 'editor', 'plan']);
  assert(pb.steps.some(step => step.action === 'type' && step.el.domId === 'clear' && step.text === ''));
  const code = renderPlaywright(pb);
  assert(!code.includes('recorded-secret'));
  const run = new Function(code.replace('export default ', 'return '))();
  for (const overrides of [{}, { member: 'override-member', plan: 'Basic' }]) {
    await run(page, { password: 'replay-secret', ...overrides });
    await page.waitForURL('**/done?**');
    const result = new URL(page.url()).searchParams;
    assert.equal(result.get('member'), overrides.member || member);
    assert.equal(result.get('notes').replaceAll('\r\n', '\n'), 'First line\nSecond line');
    assert.equal(result.get('clear'), '');
    assert.equal(result.get('plan'), overrides.plan || 'Plus');
    assert.equal(result.get('password'), 'replay-secret');
    assert.equal(result.get('agree'), 'on');
    assert.equal(result.get('backing'), 'chosen');
  }
  // Inspect replay before submission to verify contenteditable, which is not a form value.
  const withoutSubmit = { ...pb, steps: pb.steps.filter(step => step.el?.domId !== 'submit') };
  await new Function(renderPlaywright(withoutSubmit).replace('export default ', 'return '))()(page, { password: 'replay-secret' });
  assert.equal(await page.locator('#editor').innerText(), 'Editable description');
  console.log('recording export: real capture, hidden/custom controls, navigation, secrets, defaults and overrides replayed successfully');
} finally {
  driver?.close();
  await browser?.close();
  if (chrome && chrome.exitCode === null) {
    const exited = once(chrome, 'exit'); chrome.kill('SIGTERM'); await exited;
  }
  await new Promise(resolve => site.close(resolve));
  rmSync(scratch, { recursive: true, force: true });
}
