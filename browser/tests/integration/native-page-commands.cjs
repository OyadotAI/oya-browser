/** Production page commands exercised in Oya against hermetic fixtures, never a CDP backend. */
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { PageDriver } = require('../../src/main/actions/driver.ts');
const { World } = require('../../src/main/native/index.ts');
const { Mouse } = require('../../src/main/input/mouse.ts');
const { Keyboard } = require('../../src/main/input/keyboard.ts');

/** The command driver uses the real analyzer, pointer and keyboard; only shell services are absent. */
function commandDriver(win) {
  const view = { webContents: win.webContents };
  const world = new World({
    analyzerScript: fs.readFileSync(path.join(__dirname, '../../scripts/analyzer.js'), 'utf8'),
    worldName: 'page-command-test',
  });
  const replies = [];
  const driver = new PageDriver({
    mouse: new Mouse(),
    keyboard: new Keyboard(process.platform),
    getActiveView: () => view,
    injectScripts: (target) => world.ensure(target),
    worldEval: (target, code) => world.evaluate(target, code),
    sendResult: (...args) => replies.push(args),
    tabs: () => [{ id: 1, view }],
    activeTabId: () => 1,
  });
  return async (action, params) => {
    if (params.selector) {
      const result = await world.evaluate(view, 'analyzePage()');
      assert.equal(result.ok, true);
      const analysis = result.data;
      const element = analysis.elements.find((item) => item.domId === params.selector.slice(1));
      assert.ok(element, `Analyzer did not expose ${params.selector}: ${JSON.stringify(analysis.elements)}`);
      params = { ...params, selector: String(element.id) };
    }
    replies.length = 0;
    await driver.runPageAction('test', action, params, view);
    assert.equal(replies.length, 1, 'one answer per command');
    return replies[0];
  };
}

/** Blocked clicks and typing must not activate either the covered control or its overlay. */
async function coveredTargets(win, run) {
  for (const [action, selector] of [
    ['click', '#blocked'],
    ['type', '#blocked-input'],
  ]) {
    const reply = await run(action, { selector, text: 'secret' });
    assert.equal(reply[1], false);
    assert.equal(reply[4], 'element_covered');
  }
  assert.deepEqual(
    await win.webContents.executeJavaScript('[activations, document.querySelector("#blocked-input").value]'),
    [0, 'original'],
  );
  await win.webContents.executeJavaScript('document.querySelector("#cover").remove()');
  assert.equal((await run('click', { selector: '#blocked' }))[1], true);
  assert.deepEqual(await win.webContents.executeJavaScript('[activations, trustedClicks]'), [1, [true]]);
}

/** Both the frame document and its ancestors participate in target hit testing. */
async function coveredFrame(win, run) {
  for (const inside of [true, false]) {
    await win.webContents.executeJavaScript(`{
      const doc = ${inside ? 'document.querySelector("iframe").contentDocument' : 'document'};
      const cover = doc.createElement('div'); cover.id='frame-cover';
      cover.style='position:fixed;inset:0;background:#ccc;z-index:100';
      doc.body.append(cover);
    }`);
    const reply = await run('type', { selector: '#editor', text: 'must not appear' });
    assert.equal(reply[4], 'element_covered', JSON.stringify(reply));
    await win.webContents.executeJavaScript(
      `${inside ? 'document.querySelector("iframe").contentDocument' : 'document'}.querySelector('#frame-cover').remove()`,
    );
    assert.equal(
      await win.webContents.executeJavaScript(
        'document.querySelector("iframe").contentDocument.querySelector("#editor").innerText',
      ),
      'Old content',
    );
  }
}

/** Form submission uses native Enter, and editing replaces the frame's content rather than appending. */
async function formAndEditor(win, run) {
  assert.equal((await run('type', { selector: '#email', text: 'agent@example.test' }))[1], true);
  assert.equal((await run('press_key', { key: 'Enter' }))[1], true);
  assert.deepEqual(await win.webContents.executeJavaScript('submissions'), [
    { value: 'agent@example.test', trusted: true },
  ]);
  assert.equal((await run('type', { selector: '#editor', text: 'Edited' }))[1], true);
  assert.equal(
    await win.webContents.executeJavaScript(
      'document.querySelector("iframe").contentDocument.querySelector("#editor").innerText',
    ),
    'Edited',
  );
  assert.equal(
    await win.webContents.executeJavaScript(
      'document.querySelector("iframe").contentWindow.edits.length > 0 && document.querySelector("iframe").contentWindow.edits.every(e => e)',
    ),
    true,
  );
}

/** Local fixtures distinguish Oya command correctness from the public site's unavailable services. */
module.exports = async function checkPageCommands(win, profile) {
  const fixture = path.join(profile, 'page-commands.html');
  fs.writeFileSync(
    fixture,
    `<!doctype html><style>body{margin:20px}#blocked,#blocked-input{position:absolute;top:20px;height:40px}#blocked-input{left:200px}#cover{position:fixed;inset:0 0 auto;height:80px;background:#ccc;z-index:10}form{margin-top:110px}iframe{display:block;width:500px;height:150px;margin-top:30px}</style>
    <button id="blocked">Blocked</button><input id="blocked-input" value="original"><div id="cover"></div>
    <form><input id="email" type="email"><button>Submit</button></form>
    <iframe srcdoc="<body><div id='editor' contenteditable='true'>Old content</div><script>window.edits=[];document.addEventListener('input',e=>edits.push(e.isTrusted))</script>"></iframe>
    <script>window.activations=0;window.trustedClicks=[];window.submissions=[];document.querySelector('#blocked').onclick=e=>{activations++;trustedClicks.push(e.isTrusted)};document.querySelector('#cover').onclick=()=>activations++;document.querySelector('form').onsubmit=e=>{e.preventDefault();submissions.push({value:document.querySelector('#email').value,trusted:e.isTrusted})}</script>`,
  );
  await win.loadFile(fixture);
  win.focus();
  win.webContents.focus();
  const run = commandDriver(win);
  await coveredTargets(win, run);
  await coveredFrame(win, run);
  await formAndEditor(win, run);
  console.log(
    'PASS: production PageDriver refuses overlays, delivers trusted clicks/form submission and replaces iframe editor content without CDP',
  );
};
