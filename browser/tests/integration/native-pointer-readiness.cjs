/** Dynamic nested scrolling uses actual native wheel defaults after isolated layout readiness, never scripted scrolling. */
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { dispatchNativePointer } = require('../../src/main/native/pointer-input.ts');
const { capturePage } = require('../../src/main/native/index.ts');

/** A pending style/layout change must not send an otherwise correctly targeted wheel to the outer scroller. */
module.exports = async function nativePointerReadiness(win, profile, until) {
  const file = path.join(profile, 'native-pointer-readiness.html');
  fs.writeFileSync(
    file,
    '<!doctype html><div style="height:3000px"></div><script>window.wheels=[];document.addEventListener("wheel",e=>wheels.push({trusted:e.isTrusted,inside:!!e.target.closest("#scroll-pane")}));</script>',
  );
  const contents = win.webContents,
    view = { webContents: contents };
  await win.loadFile(file);
  await capturePage(view);
  await dispatchNativePointer(view, { type: 'mouseWheel', x: 10, y: 10, deltaX: 0, deltaY: 200 });
  await until(() => contents.executeJavaScript('scrollY > 0'));
  await contents.executeJavaScript(
    "window.scrollTo(0,0);const pane=document.createElement('div');pane.id='scroll-pane';pane.style='position:fixed;left:100px;top:100px;width:200px;height:200px;overflow:auto';pane.innerHTML='<div style=\"height:2000px\">Scrollable pane</div>';document.body.append(pane);document.elementFromPoint=()=>{throw Error('page hook must not run')};void 0;",
  );
  await dispatchNativePointer(view, { type: 'mouseWheel', x: 150, y: 150, deltaX: 0, deltaY: 120 });
  await until(() => contents.executeJavaScript('document.getElementById("scroll-pane").scrollTop > 0'));
  const actual = await contents.executeJavaScript(
    '({pane:document.getElementById("scroll-pane").scrollTop,outer:scrollY,wheels})',
  );
  assert.ok(actual.pane > 0, 'live scroll targets the pane under the pointer');
  assert.equal(actual.outer, 0, 'scrolling a pane leaves the outer page in place');
  assert.ok(
    actual.wheels.some((event) => event.trusted && event.inside),
    'the actual trusted native wheel reaches the nested pane',
  );
};
