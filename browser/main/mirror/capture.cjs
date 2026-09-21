/**
 * Capturing one real browser: snapshot a profile, launch the user's own
 * browser on the copy so it decrypts its own cookies, read them over CDP, and
 * kill it. The copy is why this is safe: Chrome 136+ refuses debugging on the
 * live profile, and we never touch what the user is actually running.
 */
const fs = require('fs');
const os = require('os');
const path = require('path');
const { spawn } = require('child_process');
const { sourceBrowser } = require('./locate.cjs');
const { captureDevice } = require('./device.cjs');
const { CdpWs } = require('./cdp-ws.cjs');
const { readVersion, readCookies } = require('./read.cjs');
const { copyIfPresent } = require('./copy.cjs');
const {
  LAUNCH_READY_TIMEOUT_MS,
  LAUNCH_POLL_MS,
  LAUNCH_INPUTS,
  PROFILE_INPUTS,
  PROFILE_STORES,
} = require('./constants.cjs');

/** Per-profile files copied so the launched browser can open the profile and its sessions. */
const PROFILE_COPY = [...PROFILE_INPUTS, ...PROFILE_STORES];

/** Makes an offline copy of the profile the launched browser will open. */
function snapshot(source, profileDir, scratch) {
  for (const input of LAUNCH_INPUTS) copyIfPresent(path.join(source.userDataDir, input), path.join(scratch, input));
  const from = path.join(source.userDataDir, profileDir);
  for (const item of PROFILE_COPY) copyIfPresent(path.join(from, item), path.join(scratch, profileDir, item));
  return path.join(scratch, profileDir);
}

/** The static launch flags: headless, an ephemeral debugging port, and no first-run noise. */
const LAUNCH_FLAGS = [
  '--headless=new',
  '--remote-debugging-port=0',
  '--no-first-run',
  '--no-default-browser-check',
  '--disable-sync',
];

/** Launches the user's browser on the copy, headless, with a debugging port. */
function launch(exe, scratch, profileDir) {
  const args = [`--user-data-dir=${scratch}`, `--profile-directory=${profileDir}`, ...LAUNCH_FLAGS];
  return spawn(exe, args, { stdio: 'ignore' });
}

/** The debugging port the launched browser wrote, or null until it has. */
function readPort(scratch) {
  try {
    return Number(fs.readFileSync(path.join(scratch, 'DevToolsActivePort'), 'utf8').split('\n')[0]) || null;
  } catch {
    return null;
  }
}

/** Waits for the launched browser to advertise its debugging port. */
async function waitForPort(scratch) {
  const deadline = Date.now() + LAUNCH_READY_TIMEOUT_MS;
  do {
    const port = readPort(scratch);
    if (port) return port;
    await sleep(LAUNCH_POLL_MS);
  } while (Date.now() < deadline);
  throw new Error('Launched browser never opened its debugging port');
}

/** Resolves after `ms`, without blocking a timer from letting the process exit. */
function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms).unref?.());
}

/** Snapshot, launch, read the profile's cookies and the real version, then tear down. */
async function captureProfile(scratchRoot, source, profile) {
  const scratch = fs.mkdtempSync(path.join(scratchRoot, 'oya-mirror-'));
  snapshot(source, profile.dir, scratch);
  const child = launch(source.exe, scratch, profile.dir);
  try {
    return await readOverCdp(await waitForPort(scratch), profile);
  } finally {
    teardown(child, scratch);
  }
}

/** Connects to the launched browser and reads its version and cookies. */
async function readOverCdp(port, profile) {
  const cdp = new CdpWs(port);
  await cdp.connect();
  try {
    return await profileRecord(cdp, profile);
  } finally {
    cdp.close();
  }
}

/** The captured profile: its cookies and the real Chrome version, tagged with the profile's identity. */
async function profileRecord(cdp, profile) {
  const { chromeVersion } = await readVersion(cdp);
  const cookies = await readCookies(cdp);
  return { profile: profile.dir, name: profile.name, lastUsed: !!profile.lastUsed, chromeVersion, cookies };
}

/** Kills the launched browser and deletes its scratch copy. */
function teardown(child, scratch) {
  try {
    child.kill();
  } catch {}
  fs.rmSync(scratch, { recursive: true, force: true });
}

/** The whole capture: the real device once, then each profile's cookies. Null when nothing to mirror. */
async function captureAll(ctx) {
  const source = sourceBrowser();
  if (!source) return null;
  const device = await captureDevice(ctx.electron);
  // Firefox carries its cookies already (read from SQLite); Chromium needs a
  // launch per profile to decrypt them. Firefox keeps the Electron Chrome
  // version, since its own Gecko version cannot be presented convincingly.
  const profiles = source.kind === 'firefox' ? source.profiles : await captureProfiles(ctx, source);
  if (!profiles.length) return null;
  if (profiles[0].chromeVersion) device.chromeVersion = profiles[0].chromeVersion;
  return { source: source.id, name: source.name, userDataDir: source.userDataDir, device, profiles };
}

/** Each profile captured in turn; a profile that fails is logged and skipped, not fatal. */
async function captureProfiles(ctx, source) {
  const scratchRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-mirror-root-'));
  const out = [];
  for (const p of source.profiles) await pushCapture(out, scratchRoot, source, p);
  return out;
}

/** Captures one profile into `out`; a profile that fails is logged and skipped, never fatal. */
async function pushCapture(out, scratchRoot, source, p) {
  try {
    out.push(await captureProfile(scratchRoot, source, p));
  } catch (e) {
    console.log(`[oya] Mirror skipped profile ${p.dir}: ${e.message}`);
  }
}

module.exports = { captureAll };
