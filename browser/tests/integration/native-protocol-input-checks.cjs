/** Real trusted pointer and keyboard effects through Oya's external adapter, never synthetic DOM events. */
const assert = require('node:assert/strict');
/** Observe actual renderer state rather than counting accepted dispatches. */
async function until(wc, expression) {
  for (let i = 0; i < 100; i++) {
    if (await wc.executeJavaScript(expression)) return;
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
  throw Error('Native protocol input did not reach the page: ' + expression);
}
/** Exercise form edits, trusted events, held buttons, wheel direction and CSS-to-native zoom mapping. */
module.exports = async function input(a, wc, control) {
  await wc.executeJavaScript(`document.body.innerHTML='<style>body{margin:0;height:3000px}button{position:absolute;left:20px;top:20px;width:120px;height:40px}input{position:absolute;left:20px;top:100px;width:150px}</style><button>Click</button><input>';
    window.inputEvents=[]; for(const type of ['click','mousedown','mouseup','pointermove','wheel','keydown','keypress','keyup','input'])
    document.addEventListener(type,e=>inputEvents.push({type,trusted:e.isTrusted,buttons:e.buttons,button:e.button,x:e.clientX,y:e.clientY,key:e.key,shift:e.shiftKey,deltaY:e.deltaY}));`);
  const mouse = (type, extra = {}) => a.call('Input.dispatchMouseEvent', { type, x: 60, y: 40, ...extra });
  const click = async () => {
    await mouse('mousePressed', { button: 'left', clickCount: 1 });
    await mouse('mouseReleased', { button: 'left', clickCount: 1 });
  };
  for (const zoom of [1, 2]) {
    wc.setZoomFactor(zoom);
    await wc.executeJavaScript('window.inputEvents=[];scrollTo(0,0)');
    await mouse('mouseMoved', { x: 60.25 });
    await click();
    await until(wc, 'inputEvents.some(e=>e.type==="click")');
    const events = await wc.executeJavaScript('inputEvents');
    assert.ok(events.every((e) => e.trusted));
    const pressed = events.find((e) => e.type === 'mousedown');
    assert.equal(pressed.buttons, 1);
    assert.equal(pressed.x, 60);
    assert.equal(pressed.y, 40);
    assert.equal(events.find((e) => e.type === 'mouseup').buttons, 0);
    assert.equal(events.find((e) => e.type === 'pointermove').button, -1);
    assert.equal(events.find((e) => e.type === 'pointermove').x, 60.25);
    await mouse('mousePressed', { button: 'left', clickCount: 1 });
    await mouse('mouseMoved', { x: 80, buttons: 1 });
    await mouse('mouseReleased', { x: 80, button: 'left', clickCount: 1 });
    await until(wc, 'inputEvents.some(e=>e.type==="pointermove"&&e.buttons===1)');
    await mouse('mouseWheel', { x: 250, y: 180, deltaX: 0, deltaY: 80 });
    await until(wc, 'inputEvents.some(e=>e.type==="wheel")');
    assert.equal((await wc.executeJavaScript('inputEvents.find(e=>e.type==="wheel")')).deltaY, 80);
    await until(wc, 'scrollY>0');
  }
  wc.setZoomFactor(1);
  await wc.executeJavaScript('scrollTo(0,0);document.querySelector("input").focus();window.inputEvents=[]');
  const key = (type, extra = {}) => a.call('Input.dispatchKeyEvent', { type, ...extra });
  await key('rawKeyDown', { key: 'a', code: 'KeyA', windowsVirtualKeyCode: 65 });
  await key('char', { text: 'a' });
  await key('keyUp', { key: 'a' });
  await key('keyDown', { key: 'B', code: 'KeyB', text: 'B', modifiers: 8 });
  await key('keyUp', { key: 'B', modifiers: 8 });
  await until(wc, 'document.querySelector("input").value==="aB"');
  await key('keyDown', { key: 'Backspace', code: 'Backspace', windowsVirtualKeyCode: 8 });
  await key('keyUp', { key: 'Backspace' });
  await until(wc, 'document.querySelector("input").value==="a"');
  assert.ok((await wc.executeJavaScript('inputEvents')).every((e) => e.trusted));
  await a.call('Input.insertText', { text: '日本語 🚀' });
  await until(wc, 'document.querySelector("input").value==="a日本語 🚀"');
  control.localHeld = true;
  try {
    await assert.rejects(click(), /human/);
    await assert.rejects(key('keyDown', { key: 'Enter' }), /human/);
  } finally {
    control.localHeld = false;
  }
  await assert.rejects(mouse('mousePressed', { button: 'back' }), /Unsupported/);
  await assert.rejects(key('keyDown', { key: 'a', code: 'KeyB' }), /physical/);
  await assert.rejects(key('char', { text: '🚀' }), /Unicode|composed/);
  await wc.loadURL(wc.getURL());
  console.log(
    'PASS native external input: trusted clicks, drag button state, CSS zoom coordinates, wheel deltas, physical keys, edits, Unicode and human admission',
  );
};
