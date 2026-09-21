/**
 * Real Electron check of the desktop shell: setup, recording, review, save,
 * export, dialogs, themes, shortcuts and four window layouts, with
 * screenshots. Run: npm run test:shell. Code passed to page.evaluate runs in
 * the shell page.
 */
/* global window, document, getComputedStyle, requestAnimationFrame, innerWidth, Chat, NetLog */
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createServer } from 'node:http';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { _electron as electron } from '../../../ui/node_modules/playwright/index.mjs';
import { shellLayout } from '../../shell-layout.cjs';

const profile = await mkdtemp(join(tmpdir(), 'oya-shell-test-'));
const output = process.env.OYA_SHELL_SCREENSHOTS || join(tmpdir(), 'oya-desktop-redesign');
await mkdir(output, { recursive: true });
const fixture = `<!doctype html><meta charset="utf-8"><title>Member lookup · Northline</title><style>
body{margin:0;background:#f8fafb;color:#263443;font:14px -apple-system,system-ui}header{padding:24px 36px;background:white;border-bottom:1px solid #e2e7eb;font-weight:600}header span{float:right;color:#73808c;font-size:12px}main{max-width:620px;margin:64px auto;padding:0 32px}small{color:#657986}h1{font-size:30px;letter-spacing:-1px}p{color:#73808c;line-height:1.6}form{margin-top:32px;background:white;padding:28px;border:1px solid #e2e7eb;border-radius:12px}label{display:block;margin-bottom:10px;font-size:12px}input{display:block;width:90%;padding:12px;border:1px solid #ccd6dd;border-radius:6px;margin-bottom:20px}button{padding:12px 22px;background:#254863;border:0;border-radius:6px;color:white}</style>
<header>Northline <span>Provider workspace</span></header><main><small>MEMBER SERVICES / LOOKUP</small><h1>Find a member</h1><p>Enter the member information to review their coverage and available benefits.</p><form><label for="member">Member ID</label><input id="member" name="member" placeholder="Enter member ID"><label for="dob">Date of birth</label><input id="dob" name="dob" placeholder="MM / DD / YYYY"><button type="button">Continue</button></form></main>`;
const site = createServer((req, res) => {
  if (req.url === '/slow') {
    res.on('close', () => res.destroy());
    return;
  }
  if (req.url === '/fail') {
    res.socket.destroy();
    return;
  }
  // No background of its own, the canvas underneath is whatever the browser paints.
  if (req.url === '/bare') {
    res.setHeader('Content-Type', 'text/html');
    res.end('<!doctype html><title>Bare page</title><p>No background here.');
    return;
  }
  res.setHeader('Content-Type', 'text/html');
  res.end(fixture);
});
let application;
try {
  await new Promise((resolve) => site.listen(0, '127.0.0.1', resolve));
  const url = `http://127.0.0.1:${site.address().port}/`;
  application = await electron.launch({
    executablePath:
      process.env.OYA_TEST_EXECUTABLE || createRequire(import.meta.url)('../../launch.cjs').developmentExecutable(),
    args: [fileURLToPath(new URL('../../main.js', import.meta.url))],
    cwd: fileURLToPath(new URL('../../', import.meta.url)),
    env: {
      ...process.env,
      OYA_USER_DATA_DIR: profile,
      OYA_API_KEY: '',
      OYA_AUTO_CONNECT: 'false',
      OYA_SERVER_URL: '',
      OYA_REMOTE_DEBUGGING_PORT: '0',
    },
  });
  const page = await application.firstWindow();
  assert.equal(await application.evaluate(({ app }) => app.getName()), 'Oya Browser');
  if (process.platform === 'darwin') {
    const menu = await application.evaluate(({ Menu }) => {
      const item = Menu.getApplicationMenu().items[0];
      return { label: item.label, children: item.submenu.items.map((child) => child.label) };
    });
    assert.equal(menu.label, 'Oya Browser');
    assert(menu.children.includes('About Oya Browser') && menu.children.includes('Quit Oya Browser'));
  }
  page.setDefaultTimeout(6000);
  const errors = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.waitForFunction(() => typeof window.shellIcon === 'function');
  await page.locator('#btn-manual').click();
  // CDP clicks bypass native draggable regions: check those separately so a
  // passing automation test cannot conceal a shell that swallows human clicks.
  for (const selector of [
    'body',
    '.setup-screen',
    '.setup-screen .setup-card',
    '#cfg-server',
    '#cfg-key',
    '#cfg-name',
    '#btn-connect',
    '#btn-signin',
  ]) {
    assert.equal(
      await page.locator(selector).evaluate((el) => getComputedStyle(el).getPropertyValue('-webkit-app-region')),
      'no-drag',
      `${selector} must accept native input`,
    );
  }
  const originalBrowserName = await page.locator('#cfg-name').inputValue();
  await page.locator('#cfg-name').fill('');
  await page.locator('#cfg-name').click();
  await page.keyboard.type('Input check');
  assert.equal(await page.locator('#cfg-name').inputValue(), 'Input check');
  await page.locator('#cfg-name').fill(originalBrowserName);
  await application.evaluate(({ nativeTheme }) => {
    nativeTheme.themeSource = 'light';
  });
  await page.waitForFunction(() => document.documentElement.dataset.theme === 'light');
  const capture = async (name) => {
    await page.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    // Let transitions and entrances settle (never waiting on the endless ones, such as a spinner).
    await page.evaluate(() => {
      const settling = document.getAnimations().filter((a) => a.effect?.getTiming().iterations !== Infinity);
      const cap = new Promise((resolve) => setTimeout(resolve, 1500));
      return Promise.race([Promise.all(settling.map((a) => a.finished.catch(() => {}))), cap]);
    });
    const png = await application.evaluate(async ({ BrowserWindow }) =>
      (await BrowserWindow.getAllWindows()[0].capturePage()).toPNG().toString('base64'),
    );
    await writeFile(join(output, name + '.png'), Buffer.from(png, 'base64'));
  };
  await capture('setup-light');
  await application.evaluate(({ nativeTheme }) => {
    nativeTheme.themeSource = 'dark';
  });
  await page.waitForFunction(() => document.documentElement.dataset.theme === 'dark');
  await capture('setup-dark');
  await application.evaluate(({ nativeTheme }) => {
    nativeTheme.themeSource = 'light';
  });
  await page.waitForFunction(() => document.documentElement.dataset.theme === 'light');
  await page.locator('#cfg-server').fill('invalid');
  await page.locator('#btn-connect').click();
  assert.match(await page.locator('#setup-error').innerText(), /valid/);
  await capture('connection-validation-light');
  await application.evaluate(({ app }, url) => {
    app.on('web-contents-created', (_event, contents) => {
      contents.session.webRequest.onBeforeRequest(
        { urls: ['https://google.com/*', 'https://www.google.com/*'] },
        (_details, callback) => callback({ redirectURL: url }),
      );
    });
  }, url);
  await page.locator('#btn-skip').click();
  await page.getByRole('tab', { name: 'Member lookup · Northline', exact: true }).waitFor();
  await application.evaluate(async ({ BrowserWindow }) => {
    const contents = BrowserWindow.getAllWindows()[0].getBrowserView().webContents;
    if (contents.isLoading()) await new Promise((resolve) => contents.once('did-stop-loading', resolve));
  });
  // A page that paints no background must not show the dark shell colour through.
  await application.evaluate(({ nativeTheme }) => {
    nativeTheme.themeSource = 'dark';
  });
  await page.waitForFunction(() => document.documentElement.dataset.theme === 'dark');
  await page.locator('#url-bar').fill(url + 'bare');
  await page.locator('#url-bar').press('Enter');
  await page.locator('#navigation-progress').waitFor({ state: 'hidden' });
  const canvas = await application.evaluate(async ({ BrowserWindow }) => {
    const image = await BrowserWindow.getAllWindows()[0].getBrowserView().webContents.capturePage();
    return [...image.toBitmap().subarray(0, 3)].reverse(); // BGRA, top-left pixel
  });
  assert.deepEqual(canvas, [255, 255, 255], 'a page without its own background paints white, not the window colour');
  await application.evaluate(({ nativeTheme }) => {
    nativeTheme.themeSource = 'light';
  });
  await page.waitForFunction(() => document.documentElement.dataset.theme === 'light');
  await page.locator('#url-bar').fill(url);
  await page.locator('#url-bar').press('Enter');
  await page.locator('#navigation-progress').waitFor({ state: 'hidden' });

  await page.getByRole('button', { name: 'Oya Agent', exact: true }).click();
  await page.locator('#pane-chat').waitFor({ state: 'visible' });
  await page.getByRole('tab', { name: 'Record', exact: true }).click();
  await page.locator('#pane-record').waitFor({ state: 'visible' });
  await page.waitForFunction(() => document.documentElement.dataset.panelMoving === 'false');
  // Reverse an expansion mid-flight and verify that the native page and shell
  // still finish at the same boundary. Also exercise the compact bottom panel.
  for (const width of [1280, 800]) {
    await application.evaluate(
      ({ BrowserWindow }, width) => BrowserWindow.getAllWindows()[0].setContentSize(width, 860),
      width,
    );
    await page.waitForFunction((width) => innerWidth === width, width);
    await page.evaluate(() => {
      window.motionFrames = [];
      window.oyaBrowser.onShellLayout((layout) => window.motionFrames.push(layout));
      return window.oyaBrowser.toggleDevPanel();
    });
    await page.waitForFunction(() => window.motionFrames.some((frame) => frame.progress > 0 && frame.progress < 1));
    await page.evaluate(() => window.oyaBrowser.toggleDevPanel());
    await page.waitForFunction(() => window.motionFrames.at(-1)?.progress === 1);
    assert.deepEqual(
      await application.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0].getBrowserView().getBounds()),
      shellLayout(width, 860, true).page,
    );
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await page.evaluate(() => {
      window.motionFrames = [];
      return window.oyaBrowser.toggleDevPanel();
    });
    await page.waitForFunction(() => window.motionFrames.at(-1)?.progress === 0);
    assert(await page.locator('#dev-panel').isHidden(), 'collapsed panel leaves no interactive surface');
    assert(await page.locator('#dev-panel').evaluate((el) => el.inert), 'closing disables panel interaction');
    await page.evaluate(() => window.oyaBrowser.toggleDevPanel());
    await page.waitForFunction(() => window.motionFrames.at(-1)?.progress === 1);
    assert(
      (await page.evaluate(() => window.motionFrames)).every((frame) => frame.progress === 0 || frame.progress === 1),
      'reduced motion skips intermediate frames',
    );
    await page.emulateMedia({ reducedMotion: 'no-preference' });
  }
  await application.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0].setContentSize(1280, 860));
  await capture('record-idle-light');
  await page.locator('#url-bar').fill(url + 'slow');
  await page.locator('#url-bar').press('Enter');
  await page.getByRole('button', { name: 'Stop loading', exact: true }).waitFor();
  assert(await page.locator('#navigation-progress').isVisible(), 'navigation gives immediate visible feedback');
  await capture('navigation-loading');
  await page.getByRole('button', { name: 'Stop loading', exact: true }).click();
  await page.locator('#navigation-progress').waitFor({ state: 'hidden' });
  await page.locator('#url-bar').fill(url + 'fail');
  await page.locator('#url-bar').press('Enter');
  await page.getByText('Load failed', { exact: true }).waitFor();
  await page.locator('#navigation-progress').waitFor({ state: 'hidden' });
  await page.locator('#url-bar').fill(url);
  await page.locator('#url-bar').press('Enter');
  await page.locator('#navigation-progress').waitFor({ state: 'hidden' });
  assert.equal(await page.locator('#navigation-status').innerText(), '');
  await page.evaluate(() => {
    window.menuReloadSentinel = true;
  });
  await application.evaluate(async ({ BrowserWindow, Menu }) => {
    const contents = BrowserWindow.getAllWindows()[0].getBrowserView().webContents;
    const loaded = new Promise((resolve) => contents.once('did-stop-loading', resolve));
    Menu.getApplicationMenu().getMenuItemById('browser-reload').click();
    await loaded;
  });
  assert(
    await page.evaluate(() => window.menuReloadSentinel && document.body.classList.contains('mode-browsing')),
    'native Reload preserves the shell',
  );
  await page.locator('#record-toggle').click();
  await page.waitForFunction(
    () =>
      !document.getElementById('record-toggle').disabled &&
      document.getElementById('pane-record').dataset.stage === 'recording',
  );
  await application.evaluate(async ({ BrowserWindow }) => {
    const wc = BrowserWindow.getAllWindows()[0].getBrowserView().webContents;
    await wc.executeJavaScript("document.getElementById('member').focus()");
    await wc.debugger.sendCommand('Input.insertText', { text: 'M-10482' });
    await wc.executeJavaScript("document.getElementById('dob').focus()");
    await wc.debugger.sendCommand('Input.insertText', { text: '06/12/1988' });
  });
  await page
    .locator('#record-count')
    .filter({ hasText: /^[2-9] steps$/ })
    .waitFor();
  await capture('recording-light');
  await page.locator('#record-toggle').click();
  await page.locator('#record-finish').waitFor({ state: 'visible' });
  assert.equal(await page.locator('#record-finish-title').innerText(), 'Recording captured');
  assert(await page.locator('#record-save').isDisabled(), 'offline save is explained and disabled');
  assert(await page.locator('#record-finish').isVisible(), 'finishing recording presents the save action immediately');
  assert.match(await page.locator('#record-finish-copy').innerText(), /saved on this device/);
  await capture('review-light');
  for (const size of [
    [800, 600],
    [600, 400],
  ]) {
    await application.evaluate(
      ({ BrowserWindow }, size) => BrowserWindow.getAllWindows()[0].setContentSize(...size),
      size,
    );
    await page.waitForFunction((width) => innerWidth === width, size[0]);
    await page.locator('#record-name').scrollIntoViewIfNeeded();
    await page.locator('#record-name').fill('compact-review');
    await page.locator('#record-save').scrollIntoViewIfNeeded();
    const rect = await page.locator('#record-save').boundingBox();
    assert(rect.y >= 88 && rect.y + rect.height <= size[1], 'save remains reachable at compact sizes');
    await capture(`review-light-${size[0]}`);
  }
  await application.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0].setContentSize(1280, 860));
  const originalCount = await page.locator('.studio-step').count();
  // Pause/resume preserves the same draft instead of replacing the recording.
  const draftBeforeResume = await page.evaluate(() => window.oyaBrowser.workspace({ type: 'get' }));
  await page.locator('#record-toggle').click();
  await page.waitForFunction(() => document.getElementById('pane-record').dataset.stage === 'recording');
  await page.locator('#record-toggle').click();
  await page.waitForFunction(() => document.getElementById('pane-record').dataset.stage === 'captured');
  const draftAfterResume = await page.evaluate(() => window.oyaBrowser.workspace({ type: 'get' }));
  assert.equal(draftAfterResume.draft.id, draftBeforeResume.draft.id);
  assert.equal(draftAfterResume.draft.steps.length, draftBeforeResume.draft.steps.length);
  assert.equal(draftAfterResume.storageError, null, 'draft encrypted on disk');
  await application.evaluate(({ dialog }) => {
    dialog.showMessageBox = async () => ({ response: 1 });
  });
  await page.locator('#record-validate').click();
  await page.waitForFunction(
    async () => (await window.oyaBrowser.workspace({ type: 'get' })).run?.status === 'succeeded',
    null,
    { timeout: 30000 },
  );
  await page.getByText('Steps completed.', { exact: true }).waitFor();
  await capture('validation-passed-light');
  const validated = await page.evaluate(() => window.oyaBrowser.workspace({ type: 'get' }));
  assert(validated.run.events.some((event) => event.status === 'passed'));
  assert(
    validated.runHistory.some((run) => run.id === validated.run.id),
    'validation history persisted',
  );
  await page.locator('[data-studio=steps]').click();
  // Keep the original native tab count for the later tab-overflow assertions.
  const baselineTabs = await page.locator('#tab-list [role=tab]').count();
  // Stub the server boundary, leaving the actual renderer state machine and IPC intact.
  await application.evaluate(({ BrowserWindow, ipcMain }) => {
    BrowserWindow.getAllWindows()[0].webContents.send('ws-status', {
      connected: true,
      browserId: 'test-browser',
      profileName: 'Work',
    });
    let attempts = 0;
    ipcMain.removeHandler('save-recording');
    ipcMain.handle('save-recording', (_event, name) =>
      ++attempts === 1
        ? { error: 'Connection interrupted. Try again.' }
        : {
            name,
            steps: 3,
            code: 'export default async function run(page) {\n  await page.getByLabel("Member ID").fill("M-10482");\n}\n',
          },
    );
  });
  await page.locator('#record-name').fill('member-lookup');
  await page.locator('#record-save').click();
  await page.getByText('Connection interrupted. Try again.', { exact: true }).waitFor();
  await capture('save-failed-light');
  assert.equal(await page.locator('.studio-step').count(), originalCount);
  await page.locator('#record-save').click();
  await page.getByText('Saved “member-lookup” to Oya.', { exact: true }).waitFor();
  await page.locator('[data-studio=code]').click();
  assert.equal(await page.locator('.studio-step').count(), originalCount, 'saved recording stays reviewable');
  await capture('export-light');
  const exportedPath = join(profile, 'export.js');
  await application.evaluate(({ dialog }, filePath) => {
    dialog.showSaveDialog = async () => ({ canceled: false, filePath });
  }, exportedPath);
  await page.locator('#record-download').click();
  await page.getByText('Playwright module exported.', { exact: true }).waitFor();
  const { readFile } = await import('node:fs/promises');
  assert.match(await readFile(exportedPath, 'utf8'), /export default/);
  await page.evaluate(() =>
    Object.defineProperty(navigator.clipboard, 'writeText', {
      configurable: true,
      value: async () => {
        throw new Error('denied');
      },
    }),
  );
  await page.locator('#record-copy').click();
  await page
    .locator('#code-result')
    .filter({ hasText: /denied/ })
    .waitFor();
  await page.locator('#btn-commands').click();
  await page.locator('#shell-overlay').waitFor({ state: 'visible' });
  assert(
    await page.locator('#page-backdrop').evaluate((el) => !el.hidden && el.naturalWidth > 0),
    'dialog preserves a decoded page preview',
  );
  assert.equal(
    await application.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0].getBrowserViews().length),
    0,
    'overlay detaches embedded page',
  );
  await page.locator('#theme-preference').selectOption('dark');
  await capture('commands-dark');
  await page.keyboard.press('Escape');
  assert.equal(
    await application.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0].getBrowserViews().length),
    1,
    'closing overlay restores page',
  );
  await capture('export-dark');
  await page.locator('#dev-panel-resize').hover();
  await page.mouse.down();
  await page.mouse.move(840, 350, { steps: 4 });
  await page.mouse.up();
  await page.waitForFunction(() => document.getElementById('dev-panel').getBoundingClientRect().width === 440);
  assert.equal(
    await application.evaluate(
      ({ BrowserWindow }) => BrowserWindow.getAllWindows()[0].getBrowserView().getBounds().width,
    ),
    840,
  );
  await page.evaluate(() => window.oyaBrowser.resizeDevPanel(360));
  await page.getByRole('tab', { name: 'Ask', exact: true }).click();
  await capture('ask-dark');
  // A reply whose run acted on the page offers to save it; only the latest run can be saved.
  await page.evaluate(() => Chat.reply('Searched.', [{ name: 'analyze_page' }, { name: 'type' }]));
  await page.getByRole('button', { name: 'Save as playbook', exact: true }).click();
  assert.equal(await page.getByRole('textbox', { name: 'Playbook name' }).inputValue(), 'agent-run');
  await page.getByRole('button', { name: 'Save', exact: true }).click();
  await page.locator('.chat-save-error', { hasText: 'Not connected to server' }).waitFor();
  await page.evaluate(() => Chat.reply('Nothing to replay.', [{ name: 'analyze_page' }]));
  assert.equal(await page.locator('.chat-save').count(), 0, 'a read-only run offers no playbook');
  // Step edits through the editor's More menu, then Undo and Redo.
  await page.getByRole('tab', { name: 'Record', exact: true }).click();
  await page.locator('[data-studio=steps]').click();
  const stepsBefore = await page.locator('.studio-step').count();
  await page.locator('.studio-step').nth(1).click();
  const heading = () => page.locator('#step-editor h3').innerText();
  assert.match(await heading(), /^Step 2 · /);
  await page.locator('#step-editor .editor-more > summary').click();
  await page.locator('#step-editor .editor-menu').getByText('Duplicate', { exact: true }).click();
  await page.waitForFunction((n) => document.querySelectorAll('.studio-step').length === n + 1, stepsBefore);
  await page.locator('#step-editor [aria-label="Move up"]').click();
  await page.waitForFunction(() => document.querySelector('#step-editor h3').textContent.startsWith('Step 1 · '));
  await page.locator('#step-undo').click();
  await page.waitForFunction(() => document.querySelector('#step-editor h3').textContent.startsWith('Step 2 · '));
  await page.locator('#step-redo').click();
  await page.waitForFunction(() => document.querySelector('#step-editor h3').textContent.startsWith('Step 1 · '));
  await page.locator('#step-editor .editor-more > summary').click();
  await page.locator('#step-editor .editor-menu').getByText('Delete', { exact: true }).click();
  await page.waitForFunction((n) => document.querySelectorAll('.studio-step').length === n, stepsBefore);
  // Inspect: Activity says when it is empty, then shows a row; Source names the page; the tab remembers its view.
  await page.locator('.dev-panel-header').getByRole('tab', { name: 'Inspect', exact: true }).click();
  await page.locator('[data-inspect=network]').click();
  await page.evaluate(() => NetLog.clear());
  assert(await page.locator('#net-empty').isVisible(), 'an empty activity log says what will show up');
  await application.evaluate(({ BrowserWindow }) =>
    BrowserWindow.getAllWindows()[0].webContents.send('dev-log', {
      ts: Date.now(),
      dir: 'in',
      type: 'cmd: click',
      data: '{}',
    }),
  );
  await page.locator('#net-log .dev-entry').first().waitFor();
  assert(!(await page.locator('#net-empty').isVisible()), 'the empty note goes once a message arrives');
  await page.locator('[data-inspect=source]').click();
  await page.locator('#source-refresh').click();
  await page
    .locator('#source-url')
    .filter({ hasText: /^Read from http:\/\/127\.0\.0\.1/ })
    .waitFor();
  await page.getByRole('tab', { name: 'Record', exact: true }).click();
  await page.locator('.dev-panel-header').getByRole('tab', { name: 'Inspect', exact: true }).click();
  assert(
    await page.locator('#pane-source').evaluate((el) => el.classList.contains('active')),
    'Inspect reopens Source',
  );
  // Start after a save begins a fresh draft from the current page, never a resume of the saved one.
  await page.getByRole('tab', { name: 'Record', exact: true }).click();
  const savedDraft = (await page.evaluate(() => window.oyaBrowser.workspace({ type: 'get' }))).draft.id;
  await page.locator('#record-clear').click();
  await page.waitForFunction(() => document.getElementById('pane-record').dataset.stage === 'empty');
  await page.locator('#record-toggle').click();
  await page.waitForFunction(() => document.getElementById('pane-record').dataset.stage === 'recording');
  await page.locator('#record-toggle').click();
  await page.waitForFunction(() => document.getElementById('pane-record').dataset.stage !== 'recording');
  const fresh = (await page.evaluate(() => window.oyaBrowser.workspace({ type: 'get' }))).draft;
  assert.notEqual(fresh.id, savedDraft, 'Start made a new draft');
  assert.equal(fresh.steps[0]?.start, true, 'a fresh recording starts where the person is');
  await page.getByRole('tab', { name: 'Ask', exact: true }).click();
  await page.evaluate(() => Chat.clear());
  await page.locator('#chat-input').fill('First line');
  await page.locator('#chat-input').press('Shift+Enter');
  await page.locator('#chat-input').pressSequentially('Second line');
  assert.equal(await page.locator('#chat-input').inputValue(), 'First line\nSecond line');
  await page.locator('.dev-panel-header').getByRole('tab', { name: 'Inspect', exact: true }).click();
  await capture('inspect-dark');
  await page.getByRole('tab', { name: 'Record', exact: true }).click();
  await page.locator('#btn-commands').click();
  await page.locator('#theme-preference').selectOption('system');
  await application.evaluate(({ nativeTheme }) => {
    nativeTheme.themeSource = 'light';
  });
  await page.waitForFunction(() => document.documentElement.dataset.theme === 'light');
  await application.evaluate(({ nativeTheme }) => {
    nativeTheme.themeSource = 'dark';
  });
  await page.waitForFunction(() => document.documentElement.dataset.theme === 'dark');
  await page.locator('#theme-preference').selectOption('dark');
  await page.keyboard.press('Escape');
  // Geometry must agree between native BrowserView and the DOM at every supported size.
  for (const [width, height] of [
    [1280, 860],
    [1024, 768],
    [800, 600],
    [600, 400],
  ]) {
    await application.evaluate(
      ({ BrowserWindow }, size) => BrowserWindow.getAllWindows()[0].setContentSize(...size),
      [width, height],
    );
    await page.waitForFunction((width) => innerWidth === width, width);
    const bounds = await application.evaluate(({ BrowserWindow }) =>
      BrowserWindow.getAllWindows()[0].getBrowserView().getBounds(),
    );
    assert.deepEqual(bounds, shellLayout(width, height, true).page);
    const panel = await page.locator('#dev-panel').boundingBox();
    assert.equal(Math.round(panel.y), width < 960 ? bounds.y + bounds.height : 88);
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true);
    await capture(`record-dark-${width}`);
  }
  await application.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0].setContentSize(1280, 860));
  // A shortcut originating inside the page must reach the shell.
  await application.evaluate(({ BrowserWindow }) => {
    const wc = BrowserWindow.getAllWindows()[0].getBrowserView().webContents;
    wc.focus();
    wc.sendInputEvent({
      type: 'keyDown',
      keyCode: 'L',
      modifiers: [process.platform === 'darwin' ? 'meta' : 'control'],
    });
    wc.sendInputEvent({ type: 'keyUp', keyCode: 'L', modifiers: [process.platform === 'darwin' ? 'meta' : 'control'] });
  });
  await page.waitForFunction(() => document.activeElement.id === 'url-bar');
  // Native tab status plus overflow and keyboard tab navigation.
  for (let index = 0; index < 7; index++)
    await page.evaluate((url) => window.oyaBrowser.newTab(url), url + '?tab=' + index);
  await page.waitForFunction(
    (expected) => document.querySelectorAll('#tab-list [role="tab"]').length === expected,
    baselineTabs + 7,
  );
  const activeTab = page.locator('#tab-list [role="tab"][aria-selected="true"]');
  await activeTab.focus();
  await page.keyboard.press('ArrowLeft');
  await page.waitForFunction(
    (expected) =>
      document
        .querySelector(`#tab-list .tab-item:nth-child(${expected}) [role="tab"]`)
        .getAttribute('aria-selected') === 'true',
    baselineTabs + 6,
  );
  await capture('many-tabs-dark');
  assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true);
  const preferences = JSON.parse(await readFile(join(profile, 'config.json'), 'utf8'));
  assert.equal(preferences.ui.theme, 'dark', 'appearance persisted without connection changes');
  assert.deepEqual(errors, [], 'no renderer errors');
  console.log(
    `Desktop shell passed: recording/review/save/retry/export, step edits, Inspect, dialogs, themes, shortcuts and four layouts. Screenshots: ${output}`,
  );
} finally {
  await application?.close();
  await new Promise((resolve) => site.close(resolve));
  await rm(profile, { recursive: true, force: true });
}
