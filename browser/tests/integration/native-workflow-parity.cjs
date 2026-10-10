/** Native workflow parity against real forms, accessibility names, frames and explicit local file snapshots. */
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
let diagnostic;
/** Inspect only disposable test documents when native frame targeting fails. */
async function frames(contents, pointerTrace) {
  console.error('Workflow pointer trace:', JSON.stringify(pointerTrace));
  for (const frame of contents.mainFrame.framesInSubtree) {
    try {
      console.error(
        'Workflow frame:',
        frame.url,
        await frame.executeJavaScript(
          `JSON.stringify({hits:window.__nativeHits,active:document.activeElement?.outerHTML,frames:[...document.querySelectorAll('iframe')].map(n=>({id:n.id,rect:n.getBoundingClientRect().toJSON()})),inputs:[...document.querySelectorAll('input')].map(n=>({id:n.id,value:n.value,rect:n.getBoundingClientRect().toJSON()}))})`,
        ),
      );
    } catch (error) {
      console.error(String(error));
    }
  }
}

/** Hermetic pages include duplicate frame URLs so routing must use exact owner identities. */
function page(url, port) {
  const trace = `<script>window.__nativeHits=[];addEventListener('mousedown',e=>__nativeHits.push({x:e.clientX,y:e.clientY,target:e.target.outerHTML,trusted:e.isTrusted}));</script>`;
  if (url.startsWith('/outer')) return trace + `<iframe id=inner src="http://127.0.0.1:${port}/child"></iframe>`;
  if (url.startsWith('/child')) return trace + '<label>Frame field<input id=framefield></label>';
  return `<style>#hovered{display:none}#hover:hover #hovered{display:block}#blocked{position:absolute;left:20px;top:450px}#cover{position:absolute;left:0;top:430px;width:300px;height:80px;background:#ddd;z-index:9}iframe{height:120px}</style>
  <label for=labelled>Full name</label><input id=labelled>
  <span id=accessible-name>Destination</span><input id=aria aria-labelledby=accessible-name>
  <button id=double ondblclick="window.doubled=event.isTrusted">Double action</button>
  <div id=hover>Hover menu<button id=hovered onclick="window.hovered=event.isTrusted">Revealed action</button></div>
  <label>Choice<select id=choice><option value=one>First</option><option value=two>Second choice</option></select></label>
  <input id=upload type=file>
  <button id=blocked onclick="window.blocked=true">Covered action</button><div id=cover>Cover</div>
  <iframe id=first src="http://localhost:${port}/outer"></iframe><iframe id=second src="http://localhost:${port}/outer"></iframe>
  <script>window.events=[];for(const name of ['keydown','input','change'])document.addEventListener(name,e=>events.push({type:name,target:e.target.id,trusted:e.isTrusted,key:e.key,ctrl:e.ctrlKey,meta:e.metaKey,shift:e.shiftKey}));</script>`;
}
/** Report the complete failed workspace event sequence so a native discrepancy is immediately actionable. */
async function successful(start, steps) {
  const run = await start(steps),
    result = await run.done;
  if (result.status !== 'succeeded') await diagnostic?.();
  assert.equal(result.status, 'succeeded', JSON.stringify(run.messages));
  return run;
}
/** Each behavior is observed in the native renderer, not merely accepted by a command facade. */
async function check({ start, step, css, url, tabs, profile, pointerTrace }) {
  diagnostic = () => frames(tabs.at(-1).view.webContents, pointerTrace);
  const navigate = () => step('go', 'navigate', { url: url + 'parity' });
  const candidate = (kind, value, rest = {}) => [{ kind, value, ...rest }];
  await successful(start, [
    navigate(),
    step('label', 'type', { candidates: candidate('label', 'Full name'), text: 'label-native' }),
    step('label-check', 'assert_value', { candidates: css('#labelled'), expected: 'label-native' }),
    step('aria', 'type', { candidates: candidate('role', 'Destination', { role: 'textbox' }), text: 'aria-native' }),
    step('aria-check', 'assert_value', { candidates: css('#aria'), expected: 'aria-native' }),
    step('chord', 'press_key', { key: process.platform === 'darwin' ? 'Meta+A' : 'Control+A' }),
    step('replace-key', 'press_key', { key: 'Backspace' }),
    step('chord-check', 'assert_value', { candidates: css('#aria'), expected: '' }),
    step('double', 'double_click', { candidates: candidate('text', 'Double action') }),
    step('hover', 'hover', { candidates: css('#hover') }),
    step('revealed', 'click', { candidates: candidate('text', 'Revealed action') }),
    step('select', 'select_option', { candidates: css('#choice'), option: 'Second choice' }),
    step('select-check', 'assert_value', { candidates: css('#choice'), expected: 'two' }),
  ]);
  assert.deepEqual(await tabs.at(-1).view.webContents.executeJavaScript('[doubled,hovered]'), [true, true]);
  assert.equal(
    await tabs
      .at(-1)
      .view.webContents.executeJavaScript('events.some(e=>e.target==="choice"&&e.type==="change"&&e.trusted===false)'),
    true,
  );
  const upload = path.join(profile, 'authorized-workflow-upload.txt');
  fs.writeFileSync(upload, 'native upload fixture');
  await successful(start, [navigate(), step('upload', 'upload_file', { candidates: css('#upload'), file: upload })]);
  assert.deepEqual(
    await tabs
      .at(-1)
      .view.webContents.executeJavaScript(
        'Promise.all([document.querySelector("#upload").files[0].name,document.querySelector("#upload").files[0].text()])',
      ),
    ['authorized-workflow-upload.txt', 'native upload fixture'],
  );
  assert.equal(
    await tabs
      .at(-1)
      .view.webContents.executeJavaScript('events.some(e=>e.target==="upload"&&e.type==="change"&&e.trusted===false)'),
    true,
  );
  await successful(start, [
    navigate(),
    step('frame', 'type', {
      frames: ['#second', '#inner'],
      candidates: candidate('label', 'Frame field'),
      text: 'exact child',
    }),
    step('frame-check', 'assert_value', {
      frames: ['#second', '#inner'],
      candidates: css('#framefield'),
      expected: 'exact child',
    }),
    step('sibling', 'assert_value', { frames: ['#first', '#inner'], candidates: css('#framefield'), expected: '' }),
  ]);
  await successful(start, [
    navigate(),
    step('next', 'navigate', { url: url + 'parity-next' }),
    step('back', 'go_back'),
    step('back-check', 'assert_url', { expected: url + 'parity' }),
    step('forward', 'go_forward'),
    step('forward-check', 'assert_url', { expected: url + 'parity-next' }),
  ]);
  const blocked = await start([navigate(), step('blocked', 'click', { candidates: css('#blocked'), timeout: 300 })]);
  assert.notEqual((await blocked.done).status, 'succeeded');
  assert.equal(await tabs.at(-1).view.webContents.executeJavaScript('typeof blocked'), 'object');
  assert.equal(await tabs.at(-1).view.webContents.executeJavaScript('globalThis.blocked === true'), false);
  console.log(
    'PASS native workflow parity: accessible roles/labels/text, chords, trusted double-click/hover, select, exact upload bytes, nested duplicate-URL cross-origin frames, native history and covered-target refusal',
  );
}
module.exports = { page, check };
