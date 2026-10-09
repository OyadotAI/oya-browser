/** Native agent keys must work under the same ownership fence as real production tabs. */
const assert = require('node:assert/strict');
const { BrowserView } = require('electron');
const { Shortcuts } = require('../../src/main/shell/shortcuts.ts');
const { insertNativeText } = require('../../src/main/native/index.ts');
const checkPageCommands = require('./native-page-commands.cjs');

/** Keep the page fenced and shell focused exactly as the production control shield does. */
module.exports = async function checkShieldedInput(win, profile) {
  const page = new BrowserView({ webPreferences: { sandbox: true, contextIsolation: true, nodeIntegration: false } });
  const shield = new BrowserView({ webPreferences: { sandbox: true, contextIsolation: true, nodeIntegration: false } });
  win.addBrowserView(page);
  page.setBounds({ x: 0, y: 0, width: 800, height: 550 });
  win.addBrowserView(shield);
  shield.setBounds(page.getBounds());
  await shield.webContents.loadURL('data:text/html,<body>Agent owns the page</body>');
  page.webContents.on('focus', () => win.webContents.focus());
  const shortcuts = new Shortcuts({ shell: { window: win }, control: { snapshot: () => ({ interactive: false }) } });
  shortcuts.install(page.webContents);
  Object.defineProperty(page.webContents, 'debugger', {
    get() {
      throw Error('Internal CDP is forbidden');
    },
  });
  try {
    await checkPageCommands(
      {
        webContents: page.webContents,
        loadFile: (...args) => page.webContents.loadFile(...args),
        focus: () => win.focus(),
      },
      profile,
    );
    await insertNativeText({ webContents: page.webContents }, ' 日本語 🙂');
    assert.equal(
      await page.webContents.executeJavaScript(
        'document.querySelector("iframe").contentDocument.querySelector("#editor").innerText',
      ),
      'Edited 日本語 🙂',
      'acknowledged Unicode reaches the focused child frame behind the shield',
    );
    assert.equal(page.webContents.isFocused(), false, 'agent typing must not remove the human focus fence');
    for (const type of ['keyDown', 'char', 'keyUp']) page.webContents.sendInputEvent({ type, keyCode: 'x' });
    await new Promise((resolve) => setTimeout(resolve, 100));
    assert.equal(
      await page.webContents.executeJavaScript(
        'document.querySelector("iframe").contentDocument.querySelector("#editor").innerText',
      ),
      'Edited 日本語 🙂',
      'unmarked keyboard input remains blocked during agent ownership',
    );
    console.log('PASS: production typing with agent ownership, shell focus and a native control shield');
  } finally {
    win.removeBrowserView(shield);
    win.removeBrowserView(page);
    shield.webContents.close();
    page.webContents.close();
  }
};
