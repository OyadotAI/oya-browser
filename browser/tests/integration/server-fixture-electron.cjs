/** Real native main-world execution for hermetic server service fixtures, on private parent-owned pipes only. */
const { app, BrowserWindow } = require('electron');
const { createInterface } = require('node:readline');
const { NativeRuntime } = require('../../src/main/native/index.ts');
const runtime = new NativeRuntime();
const profile = process.env.OYA_SERVER_FIXTURE_PROFILE;
if (!profile) throw Error('Launch through the parent-owned native fixture bridge');
app.setPath('userData', profile);
app.on('window-all-closed', () => {});
let window, actions, profileState, recording;
/** Refuse stock engines even though ordinary main-world execution alone is also available there. */
async function ready() {
  await app.whenReady();
  window = new BrowserWindow({
    show: false,
    webPreferences: { sandbox: true, contextIsolation: true, nodeIntegration: false },
  });
  const wc = window.webContents;
  Object.defineProperty(wc, 'debugger', {
    get() {
      throw Error('Internal CDP forbidden');
    },
  });
  if (typeof wc.mainFrame._runOyaRuntime !== 'function' || typeof wc._insertTextOya !== 'function')
    throw Error('This fixture requires the patched native Oya engine');
  wc.session.webRequest.onBeforeRequest((details, reply) => reply({ cancel: !local(details.url) }));
  await wc.loadURL('about:blank');
  return true;
}
/** Navigate only to local hermetic fixture servers, never an account or public website. */
async function navigate(url) {
  const parsed = new URL(url);
  if (!['http:', 'https:'].includes(parsed.protocol) || !['localhost', '127.0.0.1', '[::1]'].includes(parsed.hostname))
    throw Error('Native server fixture navigation is restricted to loopback');
  await window.webContents.loadURL(url);
  return { ok: true, data: { url: window.webContents.getURL(), title: window.webContents.getTitle() } };
}
/** Preserve the server command result shape without creating a general protocol proxy. */
async function send({ action, params = {} }) {
  if (action === 'record' && profileState) {
    recording ||= require('./server-fixture-recording.cjs')(window);
    return { ok: true, data: await recording(params.mode) };
  }
  if (action === 'navigate') return navigate(params.url);
  if (action === 'evaluate_raw') return { ok: true, data: { result: await evaluate(params.expression) } };
  if (profileState && action === 'list_tabs')
    return {
      ok: true,
      data: { tabs: [{ id: 1, active: true, url: window.webContents.getURL(), title: window.webContents.getTitle() }] },
    };
  if (['analyze', 'click', 'handle_dialog'].includes(action) || (profileState && action === 'type')) {
    actions ||= require('./server-fixture-actions.cjs')(window);
    return actions(action, params);
  }
  throw Error('Unsupported native fixture action: ' + action);
}
/** Existing service scripts execute in the exact owned frame using its native API. */
async function evaluate(expression) {
  if (!window || window.isDestroyed()) throw Error('Native fixture is closed');
  const reply = await runtime.execute({ webContents: window.webContents }, 'server-fixture', 'evaluate', {
    expression,
    returnByValue: true,
    awaitPromise: true,
  });
  if (reply.exceptionDetails) throw Error(reply.exceptionDetails.exception?.description || reply.exceptionDetails.text);
  if (reply.result?.unserializableValue) throw Error('Fixture result cannot be represented as JSON');
  return reply.result?.value;
}
/** Switch from the blank diagnostic surface to a cold authenticated native partition. */
async function initializeProfile(auth) {
  if (profileState) throw Error('Fixture profile is already initialized');
  const next = await require('./server-fixture-profile.cjs')(require('electron'), auth, local);
  window.destroy();
  profileState = next;
  window = next.window;
  actions = null;
  return true;
}
const methods = {
  ready,
  send,
  evaluate: (params) => evaluate(params.expression),
  profile_init: initializeProfile,
  profile_capture: (params) => profileState.capture(params.id),
  profile_cookies: () => profileState.cookies(),
  native_front_door: () => {
    if (!profileState) throw Error('Native fixture front door requires an initialized profile');
    actions ||= require('./server-fixture-actions.cjs')(window);
    return require('./server-fixture-front-door.cjs')(window, local, actions.driver);
  },
};
/** Responses contain only requested fixture results; no source code or credentials are logged. */
async function dispatch(line) {
  const request = JSON.parse(line);
  try {
    if (!Object.hasOwn(methods, request.method)) throw Error('Unsupported native fixture method');
    const value = await methods[request.method](request.params);
    process.stdout.write('OYA_FIXTURE ' + JSON.stringify({ id: request.id, value }) + '\n');
  } catch (error) {
    process.stdout.write('OYA_FIXTURE ' + JSON.stringify({ id: request.id, error: error.message }) + '\n');
  }
}
let queue = Promise.resolve();
const lines = createInterface({ input: process.stdin });
lines.on('line', (line) => {
  queue = queue.then(() => dispatch(line));
});
lines.on('close', () => app.exit(0));

/** Block external redirects and subrequests as well as explicitly requested external navigations. */
function local(url) {
  if (url === 'about:blank') return true;
  try {
    const parsed = new URL(url);
    return (
      ['http:', 'https:'].includes(parsed.protocol) && ['localhost', '127.0.0.1', '[::1]'].includes(parsed.hostname)
    );
  } catch {
    return false;
  }
}
