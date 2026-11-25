/** Verify responsive metrics, native PNGs and wrapped links in Oya's own runtime and a disposable profile. */
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const { app, BrowserWindow } = require('electron');
const { World, capturePage } = require('../../src/main/native/index.ts');
const {
  setViewport,
  viewportInfo,
  releaseAllViewports,
  viewportCaptureSize,
} = require('../../src/main/actions/viewport.ts');
const { findElementJs } = require('../../src/main/actions/scripts.ts');
const { PAGE_COMMANDS } = require('../../src/main/actions/page-commands.ts');
const { inLane } = require('../../src/main/control/agent-tab.ts');
const externalProfile = process.env.OYA_KEEL_VIEWPORT_PROFILE;
const profile = externalProfile || fs.mkdtempSync(path.join(os.tmpdir(), 'oya-keel-viewport-'));
app.setPath('userData', profile);
/** Synthetic-page PNGs remain available for visual review after the disposable profile is removed. */
const evidence = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-keel-viewport-evidence-'));
console.log(`Evidence: ${evidence}`);
const world = new World({
  analyzerScript: fs.readFileSync(path.resolve(__dirname, '../../scripts/analyzer.js'), 'utf8'),
  worldName: 'keel-viewport-fixture',
});
let win;
const timer = setTimeout(() => finish(1), 30000);
/** Cleanup only the fixture's surfaces/profile, leaving every personal Oya process alone. */
function finish(code) {
  clearTimeout(timer);
  releaseAllViewports();
  if (win && !win.isDestroyed()) win.destroy();
  try {
    if (!externalProfile) fs.rmSync(profile, { recursive: true, force: true });
  } catch {}
  app.exit(code);
}
/** Head scripts must start with retained metrics, before any later resize event can repair the page. */
async function assertInitialViewport(world, view, operation) {
  const metrics = await world.evaluate(
    view,
    '({width:document.documentElement.dataset.initialWidth,dpr:document.documentElement.dataset.initialDpr})',
  );
  assert.equal(metrics.width, '375', `${operation} parser-time width`);
  assert.equal(metrics.dpr, '2', `${operation} parser-time DPR`);
  console.log(JSON.stringify({ operation, initial: metrics }));
}
/** The production isolated-world and native rendering seams run on the real owned web contents. */
async function run() {
  win = new BrowserWindow({
    width: 1400,
    height: 1000,
    useContentSize: true,
    show: true,
    webPreferences: { sandbox: true, contextIsolation: true, backgroundThrottling: false },
  });
  const view = { webContents: win.webContents };
  view.webContents.debugger.attach = () => {
    throw Error('CDP is forbidden in this Oya fixture');
  };
  view.webContents.debugger.sendCommand = async () => {
    throw Error('CDP is forbidden in this Oya fixture');
  };
  const file = path.join(profile, 'fixture.html');
  fs.writeFileSync(
    file,
    '<!doctype html><script>document.documentElement.dataset.initialWidth=innerWidth;document.documentElement.dataset.initialDpr=devicePixelRatio;</script><meta name="viewport" content="width=device-width,initial-scale=1"><title>Keel viewport fixture</title><style>body{margin:0;background:#edf3f0}p{position:absolute;left:10px;top:100px;width:75px;font:16px/24px sans-serif}a{color:#145939}</style><p>Agree to <a id="privacy" href="#privacy">Privacy Policy</a></p><output id="size"></output><script>size.textContent=innerWidth+" × "+innerHeight;addEventListener("resize",()=>size.textContent=innerWidth+" × "+innerHeight)</script>',
  );
  await win.loadFile(file);
  const driver = { deps: { worldEval: (_view, expression) => world.evaluate(view, expression) } };
  await inLane('mcp-keel-native-fixture', async () => {
    const natural = await viewportInfo(driver, view);
    for (const width of [375, 768, 1280]) {
      const metrics = await setViewport(driver, view, { width, height: 812, deviceScaleFactor: 2 });
      assert.equal(metrics.width, width);
      assert.equal(metrics.height, 812);
      assert.equal(metrics.dpr, 2);
      const data = await capturePage(view, undefined, viewportCaptureSize(view));
      const png = Buffer.from(data.split(',')[1], 'base64');
      const image = { width: png.readUInt32BE(16), height: png.readUInt32BE(20) };
      fs.writeFileSync(path.join(evidence, `viewport-${width}.png`), png);
      console.log(JSON.stringify({ metrics, image, bytes: png.length }));
      assert.equal(image.width, width * natural.dpr, 'PNG width matches native capture density and effective viewport');
      assert.equal(image.height, 812 * natural.dpr, 'PNG height matches native capture density and effective viewport');
    }
    await setViewport(driver, view, { width: 375, height: 812, deviceScaleFactor: 2 });
    const loaded = new Promise((resolve) => view.webContents.once('did-finish-load', resolve));
    win.reload();
    await loaded;
    const retained = await viewportInfo(driver, view);
    assert.equal(retained.width, 375);
    await assertInitialViewport(world, view, 'reload');
    await win.loadFile(file, { query: { test: 'navigation' } });
    const navigated = await viewportInfo(driver, view);
    assert.equal(navigated.width, 375);
    await assertInitialViewport(world, view, 'navigation');
    const analysis = await world.evaluate(view, 'analyzePage({highlight:false})');
    const link = analysis.data.elements.find((row) => row.text === 'Privacy Policy');
    assert.ok(link && !link.covered, 'the wrapped link is reachable');
    const rectCount = await world.evaluate(view, `document.getElementById('privacy').getClientRects().length`);
    assert.ok(rectCount > 1, 'fixture link wraps across line boxes');
    await world.evaluate(
      view,
      `(()=>{const r=document.getElementById('privacy').getClientRects()[0];const overlay=document.createElement('div');overlay.style.cssText='position:fixed;z-index:99;background:white;left:'+r.left+'px;top:'+r.top+'px;width:'+r.width+'px;height:'+r.height+'px';document.body.append(overlay);})()`,
    );
    const refreshed = await world.evaluate(view, 'analyzePage({highlight:true})');
    const reference = refreshed.data.elements.find((row) => row.text === 'Privacy Policy');
    assert.ok(reference && !reference.covered, 'one unobstructed line remains clickable');
    let screenshot;
    driver.deps.sendResult = (_id, ok, data) => {
      assert.equal(ok, true);
      screenshot = data.screenshot;
    };
    await PAGE_COMMANDS.screenshot(driver, 'fixture', {}, view);
    assert.ok(screenshot.startsWith('data:image/png;base64,'));
    const spot = await world.evaluate(view, findElementJs(String(reference.id)));
    assert.equal(spot.data.covered, false);
    const hit = await world.evaluate(
      view,
      `document.elementFromPoint(${spot.data.x},${spot.data.y})?.closest('a')?.id`,
    );
    assert.equal(hit, 'privacy', 'native click coordinates land on an actual line box');
    view.webContents.sendInputEvent({
      type: 'mouseDown',
      button: 'left',
      clickCount: 1,
      x: spot.data.x,
      y: spot.data.y,
    });
    view.webContents.sendInputEvent({ type: 'mouseUp', button: 'left', clickCount: 1, x: spot.data.x, y: spot.data.y });
    await viewportInfo(driver, view);
    assert.equal(
      await world.evaluate(view, 'location.hash'),
      '#privacy',
      'native input clicks the reachable wrapped line',
    );
    await setViewport(driver, view, { reset: true });
    const reset = await viewportInfo(driver, view);
    assert.notEqual(reset.width, 375);
    win.setContentSize(400, 700);
    await setViewport(driver, view, { width: 1280, height: 812 });
    await assert.rejects(capturePage(view, undefined, viewportCaptureSize(view)), /does not fit/);
    await setViewport(driver, view, { reset: true });
    console.log('Oya native viewport, PNG, wrapped-link and reset checks passed');
  });
}
app
  .whenReady()
  .then(run)
  .then(
    () => finish(0),
    (error) => {
      console.error(error);
      finish(1);
    },
  );
