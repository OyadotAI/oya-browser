#!/usr/bin/env node
/**
 * Task files: `data: { resume: await file('./cv.pdf') }`.
 *
 * Three things have to hold:
 *  - validData is the trust boundary — it takes a well-formed file, refuses an oversized
 *    or non-base64 one, and never lets one into `secrets`.
 *  - UPLOAD_FILE_JS reaches a file input the page has hidden behind a styled button,
 *    which is what nearly every upload widget does and what no element id can name.
 *  - A recorded upload survives into a playbook as a variable and comes back out of the
 *    Playwright export as setInputFiles.
 *
 * The browser half is skipped when no Chrome binary is present.
 *
 * Usage: node test-upload.js
 */

import assert from 'node:assert/strict';
import { createServer } from 'http';
import { spawn } from 'child_process';
import { mkdtempSync, existsSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';

// Never write through to the deployment's real data/ directory.
process.env.OYA_DATA_DIR = mkdtempSync(join(tmpdir(), 'oya-upload-test-'));
process.env.API_KEYS = 'admin-key-upload-test';

const { validData, MAX_FILE_BYTES } = await import('../../src/app/api.ts');
const { UPLOAD_FILE_JS, dataKey } = await import('../../src/modules/agent/chat.ts');
const { sanitizeSteps, variablesOf, missingVariables, renderPlaywright } =
  await import('../../src/modules/playbooks/service.ts');
const { CDPDriver } = await import('../../src/drivers/cdp.ts');

const b64 = (s) => Buffer.from(s).toString('base64');
const resume = { file: 'cv.pdf', type: 'application/pdf', b64: b64('%PDF-1.4 pretend') };

// ── 1. The trust boundary ────────────────────────────────────────────────────

assert.ok(validData({ name: 'Ada', age: 36, resume }), 'a well-formed file is a task value');
assert.ok(validData({}), 'no values at all is fine');
assert.ok(!validData({ resume }, { files: false }), 'secrets refuses a file: redact() cannot hide one');
assert.ok(validData({ pin: '1234' }, { files: false }), 'secrets still takes strings');

assert.ok(!validData({ r: { ...resume, b64: 'not base64!' } }), 'b64 must be base64');
assert.ok(
  !validData({ r: { ...resume, b64: 'A'.repeat(Math.ceil(MAX_FILE_BYTES / 3) * 4 + 4) } }),
  `over ${MAX_FILE_BYTES} bytes is refused`,
);
assert.ok(!validData({ r: { file: 'cv.pdf', b64: resume.b64 } }), 'a file needs a type');
assert.ok(!validData({ r: { ...resume, file: '' } }), 'a file needs a name');
assert.ok(!validData({ r: { ...resume, file: 'x'.repeat(256) } }), 'a 256-character filename is refused');
assert.ok(!validData({ r: [resume] }), 'an array is not a file');
assert.ok(!validData({ r: null }), 'null is not a file');
assert.ok(!validData({ 'bad name': resume }), 'the key rule still applies');
console.log('  ✅ validData takes a file in data, refuses one in secrets, and bounds it');

// ── 2. Record → playbook → Playwright ────────────────────────────────────────

const recorded = [
  { action: 'navigate', url: 'https://example.com/apply', start: true },
  { action: 'type', el: { tag: 'input', domId: 'name' }, text: '{{name}}' },
  { action: 'upload_file', el: { tag: 'button', text: 'Choose file' }, file: '{{resume}}' },
  { action: 'click', el: { tag: 'button', text: 'Submit' } },
];
const steps = sanitizeSteps(recorded);
const upload = steps.find((s) => s.action === 'upload_file');
assert.ok(upload, 'an upload survives sanitizeSteps');
assert.equal(upload.file, '{{resume}}', 'the variable name is recorded, never the bytes');
assert.ok(!JSON.stringify(steps).includes(resume.b64), 'no base64 reaches a playbook');
assert.deepEqual(variablesOf(steps).sort(), ['name', 'resume'], 'the file is a playbook variable');
assert.deepEqual(missingVariables({ steps }, { name: 'Ada' }), ['resume'], 'replay demands the file');

// The model is taught to write {{name}} everywhere, so upload_file takes that spelling too.
assert.equal(dataKey('resume'), 'resume');
assert.equal(dataKey('{{resume}}'), 'resume');
assert.equal(dataKey('{{ resume }}'), 'resume');
assert.equal(dataKey(undefined), '', 'a missing name is not a crash');

const code = renderPlaywright({ name: 'apply', steps });
assert.match(code, /setInputFiles\(`\$\{vars\["resume"\]\}`\)/, 'the export calls setInputFiles with the variable');
assert.match(code, /input\[type="file"\]/, 'it targets the input, not the button that was recorded');
assert.match(code, /resume is a file path/, 'and says the var is a path there, not a file() value');
console.log('  ✅ an upload records as a variable, replays, and exports as setInputFiles');

// ── 3. The hidden input, in a real browser ───────────────────────────────────

const CHROME = [
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  '/usr/bin/google-chrome',
  '/usr/bin/chromium',
  '/usr/bin/chromium-browser',
].find((p) => existsSync(p));

if (!CHROME) {
  console.log('  ⏭  No Chrome binary found — skipping the page half');
  console.log('\n✅ test-upload passed');
  process.exit(0);
}

// The shape every upload widget uses: a styled button, the real input hidden behind it.
const PAGE = `<!doctype html><title>Upload fixture</title>
<form>
  <div class="field">
    <button type="button" id="picker" onclick="document.getElementById('real').click()">Choose file</button>
    <input type="file" id="real" name="attachment" style="display:none">
  </div>
</form>
<script>
  window.__changes = 0;
  document.getElementById('real').addEventListener('change', () => { window.__changes++; });
</script>`;

const site = createServer((req, res) => {
  res.writeHead(200, { 'Content-Type': 'text/html' });
  res.end(req.url === '/empty' ? '<!doctype html><title>No upload here</title><p>nothing</p>' : PAGE);
});
await new Promise((r) => site.listen(0, '127.0.0.1', r));
const siteUrl = `http://127.0.0.1:${site.address().port}/`;

const profile = mkdtempSync(join(tmpdir(), 'oya-upload-chrome-'));
const chrome = spawn(
  CHROME,
  [
    '--headless=new',
    '--remote-debugging-port=0',
    '--no-first-run',
    '--no-default-browser-check',
    `--user-data-dir=${profile}`,
    'about:blank',
  ],
  { stdio: ['ignore', 'ignore', 'pipe'] },
);

const wsUrl = await new Promise((resolve, reject) => {
  let buf = '';
  const timer = setTimeout(() => reject(new Error('Chrome did not report a DevTools endpoint')), 20000);
  chrome.stderr.on('data', (d) => {
    buf += d.toString();
    const m = buf.match(/ws:\/\/127\.0\.0\.1:\d+\/devtools\/browser\/[a-f0-9-]+/);
    if (m) {
      clearTimeout(timer);
      resolve(m[0]);
    }
  });
  chrome.on('exit', () => {
    clearTimeout(timer);
    reject(new Error('Chrome exited early'));
  });
});

let driver;
let failed = false;
const run = async (el, f = resume) => {
  const r = await driver.send('evaluate_raw', { expression: UPLOAD_FILE_JS(el, f) });
  return r.data.result;
};

try {
  driver = await new CDPDriver({ wsUrl, provider: 'chrome' }).connect();
  await driver.send('navigate', { url: siteUrl });

  // The whole point: the agent can only see the button, and the input is display:none.
  const viaButton = await run({ domId: 'picker', text: 'Choose file', tag: 'button' });
  assert.ok(viaButton.ok, `the button reaches the hidden input (got ${JSON.stringify(viaButton)})`);
  assert.equal(viaButton.field, 'attachment', 'it reports the field it filled');

  const state = await driver.send('evaluate_raw', {
    expression: `(() => { const i = document.getElementById('real'); return { n: i.files.length, name: i.files[0] && i.files[0].name, size: i.files[0] && i.files[0].size, type: i.files[0] && i.files[0].type, changes: window.__changes }; })()`,
  });
  const got = state.data.result;
  assert.equal(got.n, 1, 'exactly one file is attached');
  assert.equal(got.name, 'cv.pdf', 'the site sees the filename we gave');
  assert.equal(got.type, 'application/pdf', 'and the MIME type');
  assert.equal(got.size, Buffer.from(resume.b64, 'base64').length, 'the bytes arrive whole');
  assert.equal(got.changes, 1, 'a change event fires, so the page notices');
  console.log('  ✅ a display:none input behind a button takes the file');

  // Matching by label text, the way the analyzer's stable handles usually read.
  await driver.send('navigate', { url: siteUrl });
  assert.ok((await run({ text: 'Choose file' })).ok, 'the button is also found by its text');

  // One input on the page and no element named: the unambiguous case.
  await driver.send('navigate', { url: siteUrl });
  assert.ok((await run({})).ok, 'a lone file input needs no element id');

  // An element that is gone must fail loudly rather than quietly picking another input.
  const stale = await run({ domId: 'not-here' });
  assert.ok(
    !stale.ok && /no longer on the page/.test(stale.error),
    `a stale handle errors (got ${JSON.stringify(stale)})`,
  );

  // A page with no file input at all.
  await driver.send('navigate', { url: `${siteUrl}empty` });
  const none = await run({});
  assert.ok(!none.ok && /no file input/.test(none.error), `no input errors clearly (got ${JSON.stringify(none)})`);

  console.log('  ✅ stale handles and pages with no upload field fail loudly');
} catch (err) {
  failed = true;
  console.error(`  ❌ ${err.message}`);
} finally {
  try {
    driver?.close();
  } catch {}
  chrome.kill();
  site.close();
}

console.log(failed ? '\n❌ test-upload failed' : '\n✅ test-upload passed');
process.exit(failed ? 1 : 0);
