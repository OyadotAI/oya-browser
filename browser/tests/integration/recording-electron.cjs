/**
 * Real Oya regression for the production native desktop recording transport.
 * npm run test:recording --prefix browser (Linux CI uses xvfb-run).
 */
const { app, BrowserWindow, BrowserView } = require('electron');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const { createServer } = require('node:http');
const { NativeRecordingChannel } = require('../../src/main/recording/native-channel.ts');
const { recordingPreferences } = require('../../src/main/recording/preload.ts');
const { Keyboard } = require('../../src/main/input/keyboard.ts');
const { World, evaluateFrame } = require('../../src/main/native/index.ts');
const keyboard = new Keyboard(process.platform);
const { candidates } = require('../../src/workflow/index.ts');
const profile = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-recorder-electron-'));
app.setPath('userData', profile);
app.commandLine.appendSwitch('disable-gpu');
// Keep rendering with the screen locked or asleep: an unrendered page takes no real clicks.
app.commandLine.appendSwitch('disable-backgrounding-occluded-windows');
app.commandLine.appendSwitch('disable-renderer-backgrounding');
app.once('quit', () => fs.rmSync(profile, { recursive: true, force: true }));
const analyzerScript = fs.readFileSync(path.join(__dirname, '..', '..', 'scripts/analyzer.js'), 'utf8');
// The app's own analyzer context creation/recreation, alongside recording.
const ISOLATED_WORLD = 'test-recording-world';
let server, win, channel;
// The last step started, so a CI timeout names where it hung.
let lastStep = 'startup';
/** Await observable native readiness with a bounded fixture deadline. */
async function until(read) {
  for (let attempt = 0; attempt < 150; attempt++) {
    if (await read()) return;
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
  throw Error('Native recording fixture readiness timed out');
}
(async () => {
  await app.whenReady();
  setTimeout(() => {
    console.error(`Electron recording test timed out during: ${lastStep}`);
    app.exit(1);
  }, 60000).unref();
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
    webPreferences: { ...recordingPreferences(path.resolve(__dirname, '../..')), partition: 'recorder-test' },
  });
  win.setBrowserView(view);
  view.setBounds({ x: 0, y: 0, width: 900, height: 700 });
  await view.webContents.loadURL('about:blank');
  Object.defineProperty(view.webContents, 'debugger', {
    get() {
      throw Error('Internal CDP forbidden');
    },
  });
  app.focus({ steal: true });
  win.focus();
  view.webContents.focus();
  const context = new World({
    analyzerScript,
    worldName: ISOLATED_WORLD,
  });
  view.webContents.on('did-finish-load', () => context.ensure(view, { force: true }).catch(console.error));
  await view.webContents.loadURL(`http://127.0.0.1:${server.address().port}/start`);
  const steps = [],
    secrets = new Set();
  const start = async () => {
    channel = new NativeRecordingChannel(
      view.webContents,
      analyzerScript.replace('__OYA_ATTR__', 'data-test-recorder').replace('__OYA_RECORD__', 'false'),
      (out) => {
        steps.push(...(out.steps || []));
        for (const name of out.secrets || []) secrets.add(name);
      },
    );
    await channel.start();
  };
  const click = async (id) => {
    lastStep = `click #${id}`;
    const point = await view.webContents.executeJavaScript(
      `(() => { const r = document.getElementById(${JSON.stringify(id)}).getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2 }; })()`,
    );
    await view.webContents.capturePage();
    await view.webContents.executeJavaScript(`window.__fixtureMouseUp = new Promise((resolve,reject)=>{
      const timer=setTimeout(()=>{document.removeEventListener('mouseup',done,true);reject(Error('Native mouse release did not arrive'));},3000);
      function done(event){if(!event.isTrusted)return;clearTimeout(timer);document.removeEventListener('mouseup',done,true);resolve(true);}
      document.addEventListener('mouseup',done,true);
    }); undefined`);
    const delivered = view.webContents.executeJavaScript('window.__fixtureMouseUp');
    view.webContents.sendInputEvent({
      type: 'mouseDown',
      x: Math.round(point.x),
      y: Math.round(point.y),
      button: 'left',
      clickCount: 1,
    });
    view.webContents.sendInputEvent({
      type: 'mouseUp',
      x: Math.round(point.x),
      y: Math.round(point.y),
      button: 'left',
      clickCount: 1,
    });
    assert.equal(await delivered, true, 'the native release completes before the next fixture action');
  };
  await start();
  await view.webContents.loadURL(`http://127.0.0.1:${server.address().port}/redirect`);
  // Polling creates/uses the ordinary analyzer context; it must not switch the
  // recorder to that different, same-document world.
  await context.evaluate(view, 'analyzePage()');
  await channel.drain();
  await click('username');
  await view.webContents.insertText('fixture-user');
  await click('password');
  await view.webContents.insertText('fixture-secret');
  const loaded = new Promise((resolve) => view.webContents.once('did-finish-load', resolve));
  await click('submit');
  await loaded;
  lastStep = 'paint after form navigation';
  await view.webContents.executeJavaScript(
    'new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))',
  );
  assert.equal((await view.webContents.capturePage()).isEmpty(), false);
  await until(() => evaluateFrame(view.webContents.mainFrame, 'globalThis.__oyaDocumentRecorder?.armed === true'));
  // Stop while the next page's field is still focused (no change/blur event).
  await click('message');
  await view.webContents.insertText('final-field-value');
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
  assert.equal(view.webContents.listenerCount('ipc-message'), 0, 'native event admission ends with recording');
  const count = steps.length;
  await view.webContents.insertText('not-recorded');
  await click('message');
  assert.equal(steps.length, count, 'stop disarms the page listeners');
  await start();
  await click('message');
  await view.webContents.insertText('discard-me');
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
    await view.webContents.insertText('ignored');
  }
  await channel.drain(true);
  assert.equal(steps.length, 0, 'hidden targets do not produce edits');
  await click('custom');
  await click('checkLabel');
  await click('nativeLabel');
  await view.webContents.executeJavaScript(`document.getElementById('choice').focus()`);
  lastStep = 'native select keyboard change';
  await keyboard.press(view, 'ArrowDown');
  await until(() => view.webContents.executeJavaScript(`document.getElementById('choice').value === 'Two'`));
  await click('visible');
  await view.webContents.executeJavaScript(`document.getElementById('visible').select()`);
  await view.webContents.insertText('real edit');
  await click('vanishing');
  await view.webContents.insertText('keep this edit');
  await view.webContents.executeJavaScript(
    `document.getElementById('vanishing').remove(); document.getElementById('keyboardButton').focus()`,
  );
  await keyboard.press(view, 'Enter');
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
    <a id="notifications" href="#notifications"><span style="position:absolute;width:1px;height:1px;overflow:hidden;clip:rect(0 0 0 0)">1 new notification</span><span>1</span> <span id="notificationsLabel">Notifications</span></a>
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
  await until(() =>
    view.webContents.executeJavaScript('document.getElementById("france").getClientRects().length > 0'),
  );
  // Native input routing needs the newly shown menu in the compositor's hit-test data.
  await view.webContents.executeJavaScript(
    'new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))',
  );
  await click('france');
  await until(() => view.webContents.executeJavaScript('!document.getElementById("list")'));
  // Removed targets use a delayed recorder fallback; the next press must not cancel it.
  await until(() => steps.some((step) => step.action === 'click' && step.el?.domId === 'france'));
  await click('iconButton');
  await view.webContents.executeJavaScript(`document.getElementById('menu').focus()`);
  await keyboard.press(view, 'ArrowDown');
  await click('menuSpan');
  await click('secondCancel');
  await click('id_912');
  await click('notificationsLabel');
  await click('priceTo');
  await click('switchLabel');
  // Keys on something that is not a widget (the page, a plain block) scroll; they are not steps.
  await view.webContents.executeJavaScript(`document.getElementById('plain').focus()`);
  await keyboard.press(view, ' ');
  await keyboard.press(view, 'ArrowDown');
  const chosen = new Promise((resolve) =>
    view.webContents.once('-oya-file-chooser', (event, details, reply) => {
      event.preventDefault();
      assert.equal(details.processId, view.webContents.mainFrame.processId);
      reply([__filename]);
      resolve();
    }),
  );
  await view.webContents.executeJavaScript('document.getElementById("upload").click()', true);
  await chosen;
  await until(() => view.webContents.executeJavaScript('document.getElementById("upload").files.length === 1'));
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
  const notifications = steps.find((s) => s.action === 'click' && s.el?.domId === 'notifications');
  assert.equal(notifications?.el.stableText, 'Notifications', 'a live count is kept out of the stable name');
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
  await view.webContents.insertText('frame entry');
  await channel.stop();
  channel = null;
  const frameStep = steps.find((step) => step.action === 'type' && step.text === 'frame entry');
  assert(frameStep, 'trusted input inside a frame is captured');
  assert.deepEqual(frameStep.frames, ['iframe[id=payment]']);
  assert.equal(
    await view.webContents.executeJavaScript(
      `document.querySelector(${JSON.stringify(frameStep.frames[0])}).contentDocument.getElementById('reference').value`,
    ),
    'frame entry',
    'the native owner path resolves the exact recorded frame',
  );
  console.log(
    'Oya native recording: navigation, secrets, hidden/synthetic events, custom controls, labels, selection, field removal, keyboard submission, stop, clear and frame context passed',
  );
})()
  .catch((err) => {
    console.error('Failed during ' + lastStep, err);
    process.exitCode = 1;
  })
  .finally(async () => {
    await channel?.stop().catch(() => {});
    win?.destroy();
    server?.close();
    app.exit(process.exitCode || 0);
  });
