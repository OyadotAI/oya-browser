/** Real Oya callback ordering for a saved screen grant overlapping pending microphone OS consent. */
const assert = require('node:assert/strict');
const path = require('node:path');
const { setTimeout: pause } = require('node:timers/promises');
const { app, BrowserWindow, session, webContents } = require('electron');
const { MediaPermissions } = require('../../src/main/app/media-permissions.ts');
const { ScreenShare } = require('../../src/main/app/screen-share.ts');
const { SitePermissionStore } = require('../../src/main/app/site-permissions.ts');
const { JsonFile } = require('../../src/main/app/json-file.ts');
const { installPermissions } = require('../../src/main/identity/permissions.ts');
const profile = process.env.OYA_MEDIA_OVERLAP_PROFILE;
if (!profile) throw Error('Run desktop-media-overlap.mjs');
app.setPath('userData', profile);
app.commandLine.appendSwitch('use-fake-device-for-media-stream');
app.on('window-all-closed', () => {});
const deadline = setTimeout(() => {
  console.error('Native media overlap timed out');
  app.exit(1);
}, 30000);

/** Explicit page gesture, stopping every fake device track when granted. */
const capture = (contents, method, constraints) =>
  contents.executeJavaScript(
    `navigator.mediaDevices.${method}(${JSON.stringify(constraints)}).then(stream => {stream.getTracks().forEach(t => t.stop()); return 'granted';}, error => error.name)`,
    true,
  );

/** Installed native callbacks record only operation type and boolean decisions, never page content. */
function traceHooks(ses, trace) {
  return {
    setPermissionCheckHandler: (handler) =>
      ses.setPermissionCheckHandler((...args) => {
        const result = handler(...args);
        trace.push(['check', args[1], args[3]?.mediaType, result]);
        return result;
      }),
    setPermissionRequestHandler: (handler) =>
      ses.setPermissionRequestHandler((contents, kind, done, details) => {
        trace.push(['request', kind, details.mediaTypes]);
        handler(
          contents,
          kind,
          (answer) => {
            trace.push(['answer', kind, answer]);
            done(answer);
          },
          details,
        );
      }),
    setDisplayMediaRequestHandler: (handler) =>
      ses.setDisplayMediaRequestHandler((request, done) => {
        trace.push(['display']);
        handler(request, (answer) => {
          trace.push(['display-answer', !!answer]);
          done(answer);
        });
      }),
  };
}

/** Site and OS access are already granted; hold only the asynchronous OS acknowledgement. */
function services(window, trace) {
  const os = Promise.withResolvers();
  const entered = Promise.withResolvers();
  const state = { picks: 0, human: true, partition: 'persist:overlap' };
  const deps = {
    electron: {
      webContents,
      dialog: { showMessageBox: async () => ({ response: 1 }) },
      systemPreferences: {
        getMediaAccessStatus: () => 'granted',
        askForMediaAccess: async () => {
          trace.push(['os-pending']);
          entered.resolve();
          return os.promise;
        },
      },
    },
    shell: { window },
    tabs: { activeTabId: 1, getActiveView: () => ({ webContents: window.webContents }) },
    agentTab: { humanMay: () => state.human },
    governance: { configuration: null },
    persona: { partitionName: () => state.partition },
    sitePermissions: new SitePermissionStore(new JsonFile(() => path.join(profile, 'site-permissions.json'))),
    screenPicker: {
      pick: async () => {
        state.picks++;
        trace.push(['picker']);
        return null;
      },
      requestOSPermission: async () => {},
    },
  };
  deps.sitePermissions.grant('persist:overlap', 'https://meeting.test', ['audio', 'video', 'screen']);
  deps.mediaPermissions = new MediaPermissions(deps, 'darwin');
  return { deps, screen: new ScreenShare(deps, 'darwin'), os, entered, state };
}

/** The first share waits for its own consent; changing the requester while queued cancels it. */
async function overlap(ses, window, device, revoke) {
  const trace = [];
  const f = services(window, trace);
  installPermissions(traceHooks(ses, trace), undefined, f.deps.mediaPermissions, f.screen);
  const microphone = capture(window.webContents, 'getUserMedia', { [device]: true }).catch(() => 'document-replaced');
  await Promise.race([
    f.entered.promise,
    pause(3000).then(() => {
      throw Error('Native device did not enter OS approval');
    }),
  ]);
  let settled = false;
  const share = capture(window.webContents, 'getDisplayMedia', { video: true })
    .catch(() => 'document-replaced')
    .then((result) => {
      settled = true;
      return result;
    });
  await pause(150);
  assert.ok(
    trace.some(([kind, permission, types]) => kind === 'request' && permission === 'media' && types?.length === 0),
    'screen preflight reached the native handler',
  );
  const refusedWhilePending = settled;
  if (revoke === 'takeover') f.state.human = false;
  if (revoke === 'profile') f.state.partition = 'persist:another';
  let navigation;
  if (revoke === 'reload') {
    const started = new Promise((resolve) => window.webContents.once('did-start-navigation', resolve));
    navigation = window.loadURL('https://meeting.test/');
    await started;
  }
  f.os.resolve(true);
  if (navigation) {
    await navigation;
    await pause(50);
    assert.deepEqual(
      trace.filter(([kind]) => kind === 'answer').map((entry) => entry[2]),
      [false, false],
    );
  } else {
    if (revoke) assert.notEqual(await microphone, 'granted');
    else assert.equal(await microphone, 'granted');
    await share;
  }
  console.log(JSON.stringify({ device, revoke, trace, refusedWhilePending, picks: f.state.picks }));
  assert.equal(
    refusedWhilePending,
    false,
    'A remembered screen grant must not be refused just because microphone approval is pending',
  );
  const expected = revoke ? 0 : 1;
  assert.equal(f.state.picks, expected, 'Only the unchanged request reaches its own picker');
  f.state.human = true;
  f.state.partition = 'persist:overlap';
  await capture(window.webContents, 'getDisplayMedia', { video: true });
  assert.equal(f.state.picks, expected + 1, 'Cancellation or revocation cannot wedge the next independent share');
}

/** Real native callbacks over a disposable HTTPS protocol, never a live meeting or user profile. */
async function main() {
  await app.whenReady();
  const ses = session.fromPartition('persist:overlap');
  ses.protocol.handle('https', () => new Response('<!doctype html><title>Owned media test</title>'));
  const window = new BrowserWindow({ show: false, webPreferences: { session: ses, sandbox: true } });
  Object.defineProperty(window.webContents, 'debugger', {
    get() {
      throw Error('No debugger in media tests');
    },
  });
  await window.loadURL('https://meeting.test/');
  for (const device of ['audio', 'video']) await overlap(ses, window, device);
  for (const reason of ['takeover', 'profile', 'reload']) await overlap(ses, window, 'audio', reason);
  window.destroy();
}
main().then(
  () => {
    clearTimeout(deadline);
    app.exit(0);
  },
  (error) => {
    console.error(error);
    clearTimeout(deadline);
    app.exit(1);
  },
);
