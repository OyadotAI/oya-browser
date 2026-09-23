/**
 * closeTab() recreates a tab when it removes the last one, so any bulk close
 * that loops on `tabs.length` spins forever, one BrowserView per turn until
 * the app dies. That froze the desktop app on sign-in. Guard the contract.
 *
 * Run: npm test (from browser/)
 */
const assert = require('assert');
const fs = require('fs');
const path = require('path');
// The main process: main.js plus the modules it is split into, at any depth.
const root = path.join(__dirname, '..', '..');
const src = [
  'main.js',
  ...fs
    .readdirSync(path.join(root, 'main'), { recursive: true })
    .filter((f) => f.endsWith('.cjs'))
    .sort()
    .map((f) => path.join('main', f)),
]
  .map((f) => fs.readFileSync(path.join(root, f), 'utf8'))
  .join('\n');
const has = (re, msg) => assert.ok(re.test(src), msg);

// The bulk close must opt out of the "always keep one tab" rule.
const bulk = src.match(/while \(tabs\.length\) [\w.]*closeTab\([^\n]*\)/g) || [];
assert.ok(bulk.length, 'bulk close loop not found, did closeTab move?');
for (const call of bulk) {
  assert.ok(/keepOne: false/.test(call), `bulk close would never terminate: ${call}`);
}

// ...and closeTab must actually honour that opt-out.
has(/(?:function |^\s+)closeTab\(id, \{ keepOne = true \} = \{\}\) \{/m, 'closeTab lost its keepOne parameter');
has(/if \(keepOne\) (?:this\.)?createTab\(/, 'closeTab recreates unconditionally, bulk close loops forever');

// The page actions render an analysis in the saved page format, and navigate the
// way the address bar does; both need the context main.js hands them.
{
  const actions =
    fs.readFileSync(path.join(root, 'main.js'), 'utf8').match(/createPageActions\(\{[\s\S]*?\n\}\);/)?.[0] || '';
  assert.ok(
    /\bconfig: ctx\.config\b/.test(actions),
    'page actions lost the saved settings: analyze ignores the page format',
  );
  assert.ok(
    /\bnavigate: \(url\) => ctx\.tabs\.navigateActive\(url\)/.test(actions),
    'dev navigate bypasses the address bar',
  );
}

// The other half of the sign-in freeze: the jar must not go in one await at a time.
assert.ok(!/for \(const c of cookies\) \{/.test(src), 'applyCookieSync is back to a sequential await per cookie');

// A second declaration of the same name silently replaces the first, that is
// how waitForLoad(view) ended up calling waitForLoad(timeout) and resolving
// instantly instead of waiting for the page.
const names = (src.match(/^(?:async )?function [A-Za-z0-9_]+/gm) || []).map((d) =>
  d.replace(/^(?:async )?function /, ''),
);
const dupes = names.filter((n, i) => names.indexOf(n) !== i);
assert.deepStrictEqual(dupes, [], `duplicate function declarations shadow each other: ${dupes}`);

// Sends must go through wsSend, a raw send throws when the socket is down.
// A send with an error callback reports the drop instead, so it is allowed.
const rawSends = (src.match(/ws\.send\((?![^;]*=>)/g) || []).length;
assert.strictEqual(
  rawSends,
  1,
  'ws.send() outside the wsSend helper and without an error callback, a dropped socket will throw',
);

// Auto-update fails silently when the release stops shipping what the feed
// needs, no error, clients just quietly stop updating. Guard the config.
const pkg = JSON.parse(fs.readFileSync(path.join(__dirname, '..', '..', 'package.json'), 'utf8'));
const build = pkg.build || {};
// The github provider reads releases over the API and the repo was private,
// so every client got a 404 on releases.atom. The feed is served from the
// server's /downloads instead, which needs the channel files shipped there.
assert.strictEqual(build.publish?.provider, 'generic', 'update feed must not depend on a private repo');
assert.ok(/^https:\/\//.test(build.publish?.url || ''), 'publish url must be absolute');
const prodwf = fs.readFileSync(
  path.join(__dirname, '..', '..', '..', '.github', 'workflows', 'deploy-prod.yaml'),
  'utf8',
);
// Two halves now: the tag's own assets, and, for a release cut with
// `release.sh --no-desktop`, which ships no macOS build, the same files
// carried forward from the last release that had them. Miss either and mac
// clients 404 on the feed and quietly stop updating.
assert.ok(
  /for pattern in [^\n]*'latest\*\.yml'/.test(prodwf),
  'deploy must ship latest*.yml to /downloads or updates 404',
);
assert.ok(
  /carry 'latest-mac\.yml'/.test(prodwf),
  'deploy must carry latest-mac.yml forward when a release ships no desktop build',
);
assert.ok(
  /for pattern in [^\n]*'\*\.zip'/.test(prodwf),
  'deploy must ship the macOS zip to /downloads, Squirrel cannot use the dmg',
);
assert.ok(
  /carry '\*-universal\.zip'/.test(prodwf),
  'deploy must carry the macOS zip forward when a release ships no desktop build',
);
assert.ok(
  build.artifactName && !/\$\{productName\}/.test(build.artifactName),
  'artifactName must not use productName, GitHub rewrites the spaces and every update 404s',
);
assert.ok(
  (build.mac?.target || []).some((t) => t.target === 'zip'),
  'macOS needs a zip target, Squirrel.Mac cannot update from a DMG',
);
assert.ok(pkg.dependencies?.['electron-updater'], 'electron-updater must be a runtime dependency, not a devDependency');

// electron-builder publishes implicitly on a tag build once a publish config
// exists. No CI job has GH_TOKEN, so v1.0.50 died with "GitHub Personal Access
// Token is not set" and took the whole prod deploy with it.
for (const script of ['dist', 'dist:mac', 'dist:win', 'dist:linux']) {
  assert.ok(
    /--publish never/.test(pkg.scripts[script] || ''),
    `${script} must pass --publish never or a tag build tries to publish itself`,
  );
}

// Each platform's feed names its artifact, so the asset must keep that exact
// name. ${arch} renders as x86_64 for AppImage, which matched neither.
assert.strictEqual(build.linux?.artifactName, 'Oya.Browser-${version}-x64.${ext}');
assert.strictEqual(build.win?.artifactName, 'Oya.Browser-${version}-x64.${ext}');

const release = fs.readFileSync(path.join(__dirname, '..', '..', '..', 'k8s', 'scripts', 'release.sh'), 'utf8');
assert.ok(/latest-mac\.yml/.test(release), 'release.sh must publish latest-mac.yml');
assert.ok(/gh release create[^\n]*SRC_ZIP/.test(release), 'release.sh must upload the update zip');

// DAYTONA_SNAPSHOT is the deploy's fallback, so it must only ever name a
// snapshot that exists. Setting it before registration succeeds would send a
// later deploy to a snapshot that was never created, instead of leaving cloud
// browsers on the last build that worked.
assert.ok(
  /if \[ "\$CONCLUSION" = "success" \]; then\s*\n\s*gh secret set DAYTONA_SNAPSHOT/.test(release),
  'release.sh moves DAYTONA_SNAPSHOT without first confirming the snapshot was registered',
);

// Check every renderer entrypoint: a parse failure otherwise silently stops the UI.
const html = fs.readFileSync(path.join(__dirname, '..', '..', 'renderer', 'index.html'), 'utf8');
const inline = html.match(/<script(?![^>]*\bsrc=)[^>]*>([\s\S]*?)<\/script>/g) || [];
const external = [...html.matchAll(/<script src="([^"]+)"/g)].map((match) => match[1]);
assert.ok(inline.length + external.length, 'no renderer scripts found');
for (const block of inline) {
  const body = block.replace(/^<script[^>]*>/, '').replace(/<\/script>$/, '');
  assert.doesNotThrow(() => new Function(body), 'renderer inline script does not parse');
}
for (const file of external) {
  assert.doesNotThrow(
    () => new Function(fs.readFileSync(path.join(__dirname, '..', '..', 'renderer', file), 'utf8')),
    `${file} does not parse`,
  );
}

// ── Recording ──
// A recorded password must never leave the page: the step keeps a placeholder and the
// name is reported separately. Anything that buffers the raw value is the bug.
const analyzer = fs.readFileSync(path.join(__dirname, '..', '..', 'scripts', 'analyzer.js'), 'utf8');
for (const [re, msg] of [
  [/window\.__acRecordStart\b/, 'the recorder lost its start hook'],
  [/window\.__acRecordStop\b/, 'the recorder lost its stop hook'],
  [/window\.__acRecordDrain\b/, 'the recorder lost its drain hook, main.js has no way to collect steps'],
  [
    /isSecretField\(node\) \? secretPlaceholder\(node\) : value/,
    'a typed password is no longer masked before it is buffered',
  ],
  [/recordedSecrets\.add\(name\)/, 'secret field names are no longer reported, so the playbook cannot hide them'],
  [/const RECORD_ON = '__OYA_RECORD__' === 'true'/, 'the recorder cannot be armed at injection time'],
  [
    /detail === 0 && lastKey && \['Enter', ' '\]\.includes\(lastKey\.key\)/,
    'Enter on a button would record twice: the key and the click the browser makes from it',
  ],
])
  assert.ok(re.test(analyzer), msg);

// Both desktop and CDP clients must deliver events before a document disappears.
assert.ok(/new RecordingChannel\(/.test(src), 'desktop recording must use the navigation-safe event channel');
assert.ok(
  /steps: recordedSteps\.map\(\(\{ t, \.\.\.step \}\) => step\)/.test(src),
  'capture timestamps are being sent to the server as part of the steps',
);

// Pausing, browsing somewhere by hand, then resuming: without a step for that move the
// replay stays on the paused page and every later step times out looking for a target.
assert.ok(
  /pausedUrls\.set\(tab\.id, tab\.view\.webContents\.getURL\(\)\)/.test(src),
  'stopRecording no longer remembers where the pause left each tab',
);
assert.ok(
  /else if \(resumedElsewhere\(recorder, url\)\)\s*recorder\.pushRecordedStep\(\{ action: 'navigate', url \}\)/.test(
    src,
  ),
  'a resume on another page records no navigation, replay will run the rest against the paused page',
);
assert.ok(
  /if \(known\) return recorder\.pausedUrls\.get\(tabId\) !== url;/.test(src),
  'a resume no longer compares the page with where the pause left the tab',
);
assert.ok(
  /const known = recorder\.pausedDraft === recorder\.ctx\.workspace\?\.draft\.id && recorder\.pausedUrls\.has\(tabId\);/.test(
    src,
  ),
  "a resumed draft is compared with another draft's paused pages, and its move to this page goes unrecorded",
);

// A tab whose first load never settles must not wedge every later command on
// it. Unbounded awaits here made a broken browser image look like a dead server.
assert.ok(
  !/await tabs\.find\(.*?\)\?\.ready/.test(src),
  'command handler awaits tab.ready unbounded, a wedged page will hang every command',
);
assert.ok(/Promise\.race\(\[tab\.ready\.catch/.test(src), 'waitForTabReady must bound the wait');

// setupTabCDP must run after the view has a renderer (before one, CDP's Page
// domain never answers), and each attempt must be bounded so a hung command
// cannot strand the tab. Since move 7 a tab that is still not protected after
// two attempts fails closed rather than loading its page unprotected.
assert.ok(
  /const blank = tab\.view\.webContents\.loadURL\('about:blank'\)/.test(src) &&
    /withinTime\(\s*blank\.then\(\(\) => ctx\.protection\.setupTabCDP\(view\)\),\s*CDP_SETUP_TIMEOUT,?\s*\)/.test(src),
  'setupTabCDP must be bounded and run after about:blank starts the renderer, otherwise every first page is unprotected',
);
assert.ok(
  /function failClosed\(ctx, tab\) \{[\s\S]{0,200}tab\.protection = 'failed'/.test(src),
  'a tab whose protection failed twice must be marked failed, so nothing loads in it',
);

// Every page a tab loads goes through loadInTab, which refuses a tab that is
// not protected. A second loadURL of a real address would be a way around it.
const pageLoads = src.match(/\.loadURL\((?!'about:blank')/g) || [];
assert.strictEqual(
  pageLoads.length,
  1,
  `exactly one loadURL of a real address may exist in main/ (loadInTab); found ${pageLoads.length}`,
);

// Cloud browsers stream frames through viz CopyOutputResult, which needs more
// shared memory than a container's 64MB /dev/shm. Without this flag the GPU
// process dies repeatedly and Chromium SIGTRAPs the app after ~20s of streaming.
const entry = fs.readFileSync(path.join(__dirname, '..', '..', 'docker-entrypoint.sh'), 'utf8');
assert.ok(
  /electron \. .*--disable-dev-shm-usage/.test(entry),
  'container electron must run with --disable-dev-shm-usage or streaming kills the browser',
);

// Docker, Kubernetes and ECS cloud browsers have no idle stop: the server sets
// OYA_MAX_LIFETIME_MINUTES and the image is what ends the browser, so an
// abandoned one stops billing. Unset (desktop, older servers) must mean no limit.
assert.ok(
  /if \[ -n "\$\{OYA_MAX_LIFETIME_MINUTES:-\}" \]; then\n.*kill -TERM \$ELECTRON_PID/.test(entry),
  'the entrypoint must stop the browser at OYA_MAX_LIFETIME_MINUTES, and only when it is set',
);

// A persona switch must not send the previous persona's queued cookies over a
// socket already authenticated as the new one, that files one identity's
// session in another's jar, and the site then demands a fresh login.
assert.ok(
  /dropPendingCookieChanges\(\);\n\s*while \(tabs\.length\) [\w.]*closeTab/.test(src),
  'persona switch must drop queued cookie changes, not flush them into the new persona',
);

// The CDP front door re-issues every request to Chromium itself, so Chromium's
// own DNS-rebinding and CSRF defences never see the caller. Losing any of these
// three lets a page the user is visiting drive the persona's authenticated tabs.
const door = fs.readFileSync(path.join(__dirname, '..', '..', 'cdp-front-door.js'), 'utf8');
assert.ok(
  /const localHost = \(req\) => \{[\s\S]*?isIP\(host\) !== 0;/.test(door),
  'cdp-front-door lost its Host check, a rebound DNS name reaches this port same-origin',
);
assert.ok(
  /if \(!localHost\(req\)\) return send\(403/.test(door),
  'the HTTP handler no longer rejects a non-local Host',
);
assert.ok(
  /if \(!localHost\(req\) \|\| req\.headers\.origin\) return socket\.destroy\(\)/.test(door),
  'the WebSocket upgrade no longer rejects a non-local Host or a browser Origin',
);
assert.ok(
  /if \(req\.method !== 'PUT'\) return send\(405/.test(door),
  '/json/new is not PUT-only, an <img> or a form can open a tab in the persona',
);

// Sign-in popups become real windows only for the providers themselves, matched
// on the hostname: a substring match let box.com (it holds "x.com") open one.
const { isAuthPopup } = require('../../main/auth-popup.cjs');
for (const [url, want] of [
  ['https://accounts.google.com/o/oauth2/auth', true],
  ['https://api.twitter.com/oauth', true],
  ['https://github.com/login/oauth/authorize', true],
  ['https://github.com/settings', false],
  ['https://box.com/x.com', false],
  ['https://evil.test/?next=accounts.google.com', false],
  ['not a url', false],
]) {
  assert.strictEqual(isAuthPopup(url, ''), want, `isAuthPopup(${url})`);
}
assert.strictEqual(
  isAuthPopup('https://example.com', 'popup,width=500'),
  true,
  'window.open with popup features stays a window',
);

// Every IPC channel checks it was called by the shell, not a tab.
assert.ok(!/ipcMain\.handle\((?!channel)/.test(src), 'an ipcMain.handle bypasses the shell sender check, use handle()');

// ...and it must parse a bracketed IPv6 Host: a naive split on ':' reads
// "[::1]" as "[" and locks out every IPv6 loopback client.
const { localHost, isUi } = require('../../cdp-front-door');
assert(
  isUi({ type: 'page', url: 'file:///app/renderer/control-shield.html' }),
  'native input shield must never be exposed as an agent target',
);
for (const [host, want] of [
  ['127.0.0.1:9222', true],
  ['localhost:9222', true],
  ['[::1]:9222', true],
  ['[::1]', true],
  ['10.0.0.4:9222', true],
  ['rebind.attacker.test:9222', false],
  ['', false],
]) {
  assert.strictEqual(localHost({ headers: { host } }), want, `localHost(${JSON.stringify(host)})`);
}

// The server sends a proxy as a URL. A config without a host used to mean
// "direct", so every persona proxy was silently ignored.
const { normalizeProxy } = require('../../anonymity/proxy');
const fromServer = normalizeProxy({ url: 'http://gate.example.com:7000', username: 'u-session-1', password: 'pw' });
assert.deepStrictEqual(
  [fromServer.type, fromServer.host, fromServer.port, fromServer.username],
  ['http', 'gate.example.com', 7000, 'u-session-1'],
  'a server proxy URL must become host/port/type or the browser goes direct',
);
assert.strictEqual(normalizeProxy({ host: 'h', port: 1 }).host, 'h', 'a host/port proxy passes through');
assert.strictEqual(normalizeProxy(null), null, 'no proxy stays no proxy');

// Native dialogs. Page.enable is on for every tab and popup, so anything that
// enables it without a watcher wedges that surface on the first alert().
for (const enable of src.match(/(?:sendCommand|send)\('Page\.enable'\)[\s\S]{0,160}/g) || []) {
  assert.ok(
    /attachDialogWatcher/.test(enable),
    'Page.enable without attachDialogWatcher nearby, that surface blocks forever on an alert()',
  );
}
// The early answer must go out BEFORE the id is parked, or sendResult drops the
// very result being sent and the caller waits out the timeout anyway.
const early = src.match(/if \(outcome !== DIALOG_HELD\) return;[\s\S]*?\n}/);
assert.ok(early, 'the dialog race in handleCommand is gone');
assert.ok(
  early[0].indexOf('sendResult(') < early[0].indexOf('answeredCommands.add('),
  'answeredCommands.add() before sendResult() makes sendResult swallow the dialog answer',
);

// Residential proxy bytes are billed per GB, so every byte both ways must be counted.
(async () => {
  const net = require('net');
  const { meter, takeProxyBytes } = require('../../anonymity/proxy');
  const echo = net.createServer((s) => s.pipe(s));
  await new Promise((r) => echo.listen(0, '127.0.0.1', r));
  const port = await meter('127.0.0.1', echo.address().port);
  assert.strictEqual(await meter('127.0.0.1', echo.address().port), port, 'one meter per gateway');
  const got = await new Promise((resolve) => {
    const c = net.connect(port, '127.0.0.1', () => c.write('hello'));
    c.once('data', (d) => {
      c.destroy();
      resolve(d.toString());
    });
  });
  assert.strictEqual(got, 'hello', 'the meter passes traffic through untouched');
  await new Promise((r) => setTimeout(r, 50));
  assert.strictEqual(takeProxyBytes(), 10, 'bytes are counted in both directions');
  assert.strictEqual(takeProxyBytes(), 0, 'and reset once reported');
  echo.close();
  console.log(
    'ok, tabs, cookies, sends, updates, renderer, tab waits, CDP setup, shm, persona isolation, the CDP front door guarded, dialogs watched, proxies applied and metered',
  );
  process.exit(0);
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
