/**
 * Real Electron regression for the dashboard's cloud/desktop browser transport.
 * npm run test:recording --prefix browser (Linux CI uses xvfb-run).
 */
const { app, BrowserWindow, BrowserView } = require('electron');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const { createServer } = require('node:http');
const { RecordingChannel } = require('../../scripts/recording.cjs');
const { candidates } = require('../../scripts/workflow.cjs');
const { LoginState } = require('../../login-state');
const profile = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-recorder-electron-'));
app.setPath('userData', profile);
app.commandLine.appendSwitch('disable-gpu');
// Keep rendering with the screen locked or asleep: an unrendered page takes no real clicks.
app.commandLine.appendSwitch('disable-backgrounding-occluded-windows');
app.commandLine.appendSwitch('disable-renderer-backgrounding');
app.once('quit', () => fs.rmSync(profile, { recursive: true, force: true }));
const analyzerScript = fs.readFileSync(path.join(__dirname, '..', '..', 'scripts/analyzer.js'), 'utf8');
// The app's own analyzer context creation/recreation, alongside recording.
const { createWorld } = require('../../main/world.cjs');
const ISOLATED_WORLD = 'test-recording-world';
let server, win, channel;
(async () => {
  await app.whenReady();
  setTimeout(() => {
    console.error('Electron recording test timed out');
    app.exit(1);
  }, 20000).unref();
  server = createServer((req, res) => {
    if (req.url === '/redirect') {
      res.writeHead(302, { Location: `http://localhost:${server.address().port}/login` });
      return res.end();
    }
    res.setHeader('Content-Type', 'text/html');
    res.end(
      req.url.startsWith('/feed')
        ? '<!doctype html><title>Feed</title><input id="message" placeholder="Message">'
        : `<!doctype html><title>Login fixture</title><form action="/feed" method="post">
        <label>Username<input id="username" name="username"></label>
        <label>Password<input id="password" name="password" type="password"></label>
        <button id="submit">Sign in</button></form>`,
    );
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  // A visible surface is necessary for real coordinate clicks, just as the
  // cloud app runs under Xvfb. No user's browser profile or accounts are used.
  win = new BrowserWindow({ show: true, width: 920, height: 740 });
  const view = new BrowserView({
    webPreferences: { contextIsolation: true, sandbox: true, partition: 'recorder-test' },
  });
  win.setBrowserView(view);
  view.setBounds({ x: 0, y: 0, width: 900, height: 700 });
  await view.webContents.loadURL('about:blank');
  const dbg = view.webContents.debugger;
  dbg.attach('1.3');
  const sent = [];
  const send = (method, params) => {
    sent.push(method);
    return dbg.sendCommand(method, params);
  };
  const on = (method, fn) => {
    const listener = (_e, event, params) => {
      if (method === event) fn(params);
    };
    dbg.on('message', listener);
    return () => dbg.off('message', listener);
  };
  // Do not enable Runtime here: the production Electron setup doesn't.
  await send('Page.enable');
  await new LoginState().attach(send, on);
  const context = createWorld({
    cdp: (_view, method, params) => send(method, params),
    analyzerScript,
    worldName: ISOLATED_WORLD,
  });
  view.webContents.on('did-finish-load', () => context.ensureWorld(view, { force: true }).catch(console.error));
  await view.webContents.loadURL(`http://127.0.0.1:${server.address().port}/start`);
  const steps = [],
    secrets = new Set();
  const start = async () => {
    channel = new RecordingChannel({
      send,
      on,
      worldName: ISOLATED_WORLD,
      disableRuntimeOnStop: true,
      analyzer: analyzerScript.replace('__OYA_ATTR__', 'data-test-recorder').replace('__OYA_RECORD__', 'false'),
      receive: (out) => {
        steps.push(...(out.steps || []));
        for (const name of out.secrets || []) secrets.add(name);
      },
    });
    await channel.start();
  };
  const click = async (id) => {
    const point = await view.webContents.executeJavaScript(
      `(() => { const r = document.getElementById(${JSON.stringify(id)}).getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2 }; })()`,
    );
    await send('Input.dispatchMouseEvent', { type: 'mousePressed', ...point, button: 'left', clickCount: 1 });
    await send('Input.dispatchMouseEvent', { type: 'mouseReleased', ...point, button: 'left', clickCount: 1 });
  };
  await start();
  await view.webContents.loadURL(`http://127.0.0.1:${server.address().port}/redirect`);
  // Polling creates/uses the ordinary analyzer context; it must not switch the
  // recorder to that different, same-document world.
  await context.worldEval(view, 'analyzePage()');
  await channel.drain();
  await click('username');
  await send('Input.insertText', { text: 'fixture-user' });
  await click('password');
  await send('Input.insertText', { text: 'fixture-secret' });
  const loaded = new Promise((resolve) => view.webContents.once('did-finish-load', resolve));
  await click('submit');
  await loaded;
  await send('Page.captureScreenshot', { format: 'png' });
  // Stop while the next page's field is still focused (no change/blur event).
  await click('message');
  await send('Input.insertText', { text: 'final-field-value' });
  await channel.stop();
  channel = null;
  assert(
    steps.some((s) => s.action === 'type' && s.text === 'fixture-user'),
    'username captured after cross-site navigation',
  );
  assert(
    steps.some((s) => s.action === 'type' && s.text === '{{password}}'),
    'password captured as placeholder',
  );
  assert(
    steps.some((s) => s.action === 'click' && s.el.domId === 'submit'),
    'submit captured before navigation',
  );
  assert(
    steps.some((s) => s.action === 'type' && s.text === 'final-field-value'),
    'stop flushes the recorder world after navigation',
  );
  assert(!JSON.stringify(steps).includes('fixture-secret'), 'raw password never exported');
  assert(secrets.has('password'));
  assert.equal(sent.at(-1), 'Runtime.disable', 'desktop event reporting ends with recording');
  const count = steps.length;
  await send('Input.insertText', { text: 'not-recorded' });
  await click('message');
  assert.equal(steps.length, count, 'stop disarms the page listeners');
  await start();
  await click('message');
  await send('Input.insertText', { text: 'discard-me' });
  await channel.clear();
  steps.length = 0;
  secrets.clear();
  await channel.stop();
  channel = null;
  assert.equal(steps.length, 0, 'clear drops an unflushed field in the actual recorder world');
  await view.webContents.executeJavaScript(`document.body.innerHTML = \`
    <input id="visible"><input id="hidden" type="hidden">
    <div style="display:none"><input id="ancestorHidden"></div>
    <input id="transparent" style="opacity:0">
    <input id="invisible" style="visibility:hidden">
    <button id="custom" onclick="hidden.value='background'; hidden.dispatchEvent(new Event('input', {bubbles:true}))">Choose</button>
    <label id="checkLabel" for="check">Agree</label><input id="check" type="checkbox" style="display:none">
    <label id="nativeLabel" for="nativeCheck">Native</label><input id="nativeCheck" type="checkbox">
    <select id="choice" size="2"><option selected>One</option><option>Two</option></select>
    <input id="vanishing"><button id="keyboardButton">Submit</button>\``);
  await start();
  await view.webContents.executeJavaScript(`(() => {
    for (const id of ['visible', 'hidden', 'ancestorHidden', 'transparent', 'invisible']) {
      const el = document.getElementById(id); el.value = 'background';
      for (const type of ['focusin', 'input', 'change', 'click']) el.dispatchEvent(new Event(type, {bubbles:true}));
      el.dispatchEvent(new KeyboardEvent('keydown', {key:'Enter', bubbles:true}));
    }
    document.getElementById('custom').click();
  })()`);
  await channel.drain(true);
  assert.equal(steps.length, 0, 'script-dispatched events never record, even on visible fields');
  // Browser-generated focus events are trusted, even when script focuses an invisible field.
  for (const id of ['transparent', 'invisible', 'ancestorHidden', 'hidden']) {
    await view.webContents.executeJavaScript(`document.getElementById(${JSON.stringify(id)}).focus()`);
    await send('Input.insertText', { text: 'ignored' });
  }
  await channel.drain(true);
  assert.equal(steps.length, 0, 'hidden targets do not produce edits');
  await click('custom');
  await click('checkLabel');
  await click('nativeLabel');
  await view.webContents.executeJavaScript(`document.getElementById('choice').focus()`);
  await send('Input.dispatchKeyEvent', {
    type: 'keyDown',
    key: 'ArrowDown',
    code: 'ArrowDown',
    windowsVirtualKeyCode: 40,
  });
  await send('Input.dispatchKeyEvent', {
    type: 'keyUp',
    key: 'ArrowDown',
    code: 'ArrowDown',
    windowsVirtualKeyCode: 40,
  });
  await click('visible');
  await view.webContents.executeJavaScript(`document.getElementById('visible').select()`);
  await send('Input.insertText', { text: 'real edit' });
  await click('vanishing');
  await send('Input.insertText', { text: 'keep this edit' });
  await view.webContents.executeJavaScript(
    `document.getElementById('vanishing').remove(); document.getElementById('keyboardButton').focus()`,
  );
  await send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'Enter', code: 'Enter', windowsVirtualKeyCode: 13 });
  await send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Enter', code: 'Enter', windowsVirtualKeyCode: 13 });
  await channel.stop();
  channel = null;
  assert.equal(
    steps.filter((s) => s.el?.domId === 'custom').length,
    1,
    'custom control records only the visible interaction',
  );
  assert.equal(
    steps.filter((s) => s.el?.domId === 'checkLabel').length,
    1,
    'hidden checkbox records its visible label',
  );
  assert.equal(
    steps.filter((s) => s.el?.domId === 'nativeCheck').length,
    1,
    'native label activation records one checkbox click',
  );
  assert(!steps.some((s) => ['hidden', 'ancestorHidden', 'transparent', 'invisible', 'check'].includes(s.el?.domId)));
  assert(
    steps.some((s) => s.action === 'select_option' && s.option === 'Two'),
    'native keyboard selection captured',
  );
  assert.deepEqual(
    steps.filter((s) => s.action === 'type').map((s) => s.text),
    ['real edit', 'keep this edit'],
    'real edits survive field removal in order',
  );
  assert.equal(steps.filter((s) => s.action === 'press_key' && s.key === 'Enter').length, 1);
  assert(
    !steps.some((s) => s.action === 'click' && s.el?.domId === 'keyboardButton'),
    'Enter activation is not duplicated',
  );
  steps.length = 0;
  // Widgets the recorder used to miss: a transparent styled checkbox, an ARIA
  // combobox whose option removes itself on pointerdown, an icon-only button,
  // arrow keys in a list, and a hidden file input.
  await view.webContents.executeJavaScript(`document.body.innerHTML = \`
    <label><span style="position:relative;display:inline-block;width:30px;height:30px">
      <input id="styledCheck" type="checkbox" style="position:absolute;left:0;top:0;width:30px;height:30px;margin:0;opacity:0"></span>Subscribe</label>
    <div id="combo" role="combobox" tabindex="0" style="width:120px" onclick="list.style.display='block'">Country</div>
    <div id="list" role="listbox" style="display:none"><div id="france" role="option" style="width:120px">France</div></div>
    <div id="iconButton" style="cursor:pointer;width:30px;height:30px"><svg width="30" height="30"></svg></div>
    <ul id="menu" role="listbox" tabindex="0"><li role="option">A</li><li role="option">B</li></ul>
    <div id="plain" tabindex="0">Plain text</div>
    <input type="checkbox" id="id_912" name="reviews" value="344">
    <a id="priceFilter" href="#price-0-100"><span>$0.00</span> - <span id="priceTo">$99.99</span></a>
    <div style="position:relative;width:60px;height:24px"><input id="switchInput" type="checkbox" style="position:absolute;left:0;top:0;margin:0">
      <label id="switchLabel" for="switchInput" style="position:absolute;left:0;top:0;width:60px;height:24px;background:#ccc"></label></div>
    <div data-index="first"><button>Cancel</button></div><div data-index="second"><button id="secondCancel">Cancel</button></div>
    <input id="upload" type="file" style="display:none">
    <a id="menuLink" href="#" style="text-transform:uppercase"><span id="menuSpan" style="cursor:pointer">Reports</span></a>\`;
    document.getElementById('france').addEventListener('pointerdown', () => { combo.textContent = 'France'; list.remove(); });`);
  await start();
  await click('styledCheck');
  await click('combo');
  await click('france');
  await new Promise((resolve) => setTimeout(resolve, 400));
  await click('iconButton');
  await view.webContents.executeJavaScript(`document.getElementById('menu').focus()`);
  await send('Input.dispatchKeyEvent', {
    type: 'keyDown',
    key: 'ArrowDown',
    code: 'ArrowDown',
    windowsVirtualKeyCode: 40,
  });
  await send('Input.dispatchKeyEvent', {
    type: 'keyUp',
    key: 'ArrowDown',
    code: 'ArrowDown',
    windowsVirtualKeyCode: 40,
  });
  await click('menuSpan');
  await click('secondCancel');
  await click('id_912');
  await click('priceTo');
  await click('switchLabel');
  // Keys on something that is not a widget (the page, a plain block) scroll; they are not steps.
  await view.webContents.executeJavaScript(`document.getElementById('plain').focus()`);
  await send('Input.dispatchKeyEvent', {
    type: 'keyDown',
    key: ' ',
    code: 'Space',
    windowsVirtualKeyCode: 32,
    text: ' ',
  });
  await send('Input.dispatchKeyEvent', { type: 'keyUp', key: ' ', code: 'Space', windowsVirtualKeyCode: 32 });
  await send('Input.dispatchKeyEvent', {
    type: 'keyDown',
    key: 'ArrowDown',
    code: 'ArrowDown',
    windowsVirtualKeyCode: 40,
  });
  await send('Input.dispatchKeyEvent', {
    type: 'keyUp',
    key: 'ArrowDown',
    code: 'ArrowDown',
    windowsVirtualKeyCode: 40,
  });
  const { root } = await send('DOM.getDocument', {});
  const { nodeId } = await send('DOM.querySelector', { nodeId: root.nodeId, selector: '#upload' });
  await send('DOM.setFileInputFiles', { nodeId, files: [__filename] });
  await channel.stop();
  channel = null;
  const clicked = (id) => steps.filter((s) => s.action === 'click' && s.el?.domId === id).length;
  assert.equal(clicked('styledCheck'), 1, 'a transparent styled checkbox records its click');
  assert.equal(clicked('combo'), 1, 'an ARIA combobox records the click that opens it');
  assert(
    steps.some((s) => s.action === 'click' && s.el?.domId === 'france'),
    'an option that removes itself on pointerdown is still recorded',
  );
  assert.equal(clicked('iconButton'), 1, 'an icon-only pointer button records its click');
  assert.equal(
    steps.filter((s) => s.action === 'press_key' && s.key === 'ArrowDown').length,
    1,
    'arrow keys in a list are recorded, and not on a plain block',
  );
  assert(!steps.some((s) => s.action === 'press_key' && s.key === 'Space'), 'Space on a plain block is not recorded');
  assert(
    steps.some((s) => s.action === 'upload_file' && s.el?.domId === 'upload'),
    'a hidden file input records the upload',
  );
  const menu = steps.find((s) => s.action === 'click' && ['menuLink', 'menuSpan'].includes(s.el?.domId));
  assert.equal(menu?.el.domId, 'menuLink', 'a click on the span inside a link records the link');
  assert.equal(menu?.el.text, 'Reports', 'the label is the text as written, not as CSS capitalises it');
  assert.equal(menu?.el.rawHref, '#', 'the link keeps its href as written');
  const toggles = steps.filter((s) => s.action === 'click' && ['switchInput', 'switchLabel'].includes(s.el?.domId));
  assert.deepEqual(
    toggles.map((s) => s.el.domId),
    ['switchLabel'],
    'a switch whose input sits under its label records one click, on the label',
  );
  const price = steps.find((s) => s.action === 'click' && s.el?.domId === 'priceFilter');
  assert.equal(price?.el.text, '$0.00 - $99.99', 'a link whose own text is only a dash is named by all it shows');
  const row = steps.find((s) => s.action === 'click' && s.el?.domId === 'id_912');
  assert.equal(
    candidates(row?.el)[0]?.value,
    'input[name="reviews"][value="344"]',
    'a grid checkbox is found by its value',
  );
  const cancel = steps.find((s) => s.action === 'click' && s.el?.domId === 'secondCancel');
  assert.equal(
    cancel?.el.scoped,
    '[data-index="second"] button:text-is("Cancel")',
    'a repeated name is scoped to the container where it is unique',
  );
  steps.length = 0;
  await view.webContents.executeJavaScript(
    `new Promise(resolve => { const frame = document.createElement('iframe'); frame.id = 'payment'; frame.srcdoc = '<label>Reference<input id="reference"></label>'; frame.onload = resolve; document.body.replaceChildren(frame); })`,
  );
  await start();
  await view.webContents.executeJavaScript(
    `document.getElementById('payment').contentDocument.getElementById('reference').focus()`,
  );
  await send('Input.insertText', { text: 'frame entry' });
  await channel.stop();
  channel = null;
  const frameStep = steps.find((step) => step.action === 'type' && step.text === 'frame entry');
  assert(frameStep, 'trusted input inside a frame is captured');
  assert.deepEqual(frameStep.frames, ['iframe[id="payment"]']);
  console.log(
    'Electron recording: navigation, secrets, hidden/synthetic events, custom controls, labels, selection, field removal, keyboard submission, stop, clear and frame context passed',
  );
})()
  .catch((err) => {
    console.error(err);
    process.exitCode = 1;
  })
  .finally(async () => {
    await channel?.stop().catch(() => {});
    win?.destroy();
    server?.close();
    app.exit(process.exitCode || 0);
  });
