/**
 * Records the README / landing-page walkthrough against a REAL running Oya
 * stack: real browsers, real live view, real personas. Nothing is mocked.
 *
 * It films the product's argument rather than a tour of the console: a saved playbook, then
 * that playbook replaying with no model in the loop, in real time. The run that produced the
 * playbook happens before the camera rolls, because it takes about half a minute and the GIF
 * is twenty-six seconds.
 *
 *   # 1. a local stack (no database needed)
 *   OYA_UI_MODE=development API_KEYS=<key> node server/src/index.ts
 *   # 2. two or more browsers
 *   docker run -d --shm-size 2g -e OYA_SERVER_URL=ws://host.docker.internal:3100/ws \
 *     -e OYA_API_KEY=<key> -e OYA_AUTO_CONNECT=true -e OYA_BROWSER_NAME=<name> oya-browser
 *   # 3. record
 *   OYA_RECORD_KEY=<key> node scripts/record-walkthrough.mjs
 *
 * Writes ui/public/oya-browser.mp4 + oya-browser-poster.jpg,
 * assets/oya-replay-demo.gif and assets/oya-console-overview.png.
 */

import { spawn } from 'node:child_process';
import { readdirSync, mkdirSync, rmSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { chromium } from '../ui/node_modules/@playwright/test/index.mjs';

const ROOT_DIR = join(dirname(fileURLToPath(import.meta.url)), '..');
const UI_DIR = join(ROOT_DIR, 'ui');
const TEMP_VIDEO_DIR = join(ROOT_DIR, 'recorded-video');
const BASE = (process.env.OYA_RECORD_URL || 'http://localhost:3100').replace(/\/$/, '');
const KEY = process.env.OYA_RECORD_KEY;
if (!KEY) throw new Error('Set OYA_RECORD_KEY to an API key on the stack being recorded');

const SITES = [
  'https://news.ycombinator.com',
  'https://en.wikipedia.org/wiki/Web_browser',
  'https://github.com/trending',
];

/** The demonstrated task: a real public site, multi-step, and legible on screen. */
const DEMO_SITE = 'https://news.ycombinator.com';
const DEMO_TASK = [
  'Open the comments page of the top story, then return to the front page using the Hacker News logo link.',
  'Reply with one line: the title of that story.',
].join(' ');
/** The same site the README's first example uses, so the picture matches the code. */
const PLAYBOOK = 'top-story-comments';
/** Replay is shown with different inputs from the recording, because that is the point. */
const DEMO_DATA = {};
/** How long to wait on the off-camera recording pass: 5s apart, five minutes in all. */
const RUN_POLL_MS = 5000;
const RUN_POLL_TRIES = 60;
/** How long the camera stays on the replay. The GitHub flow replays in about 18 seconds. */
const REPLAY_WATCH_MS = 19_000;

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

async function api(path, body) {
  const res = await fetch(`${BASE}/api${path}`, {
    method: body ? 'POST' : 'GET',
    headers: { Authorization: `Bearer ${KEY}`, 'Content-Type': 'application/json' },
    body: body && JSON.stringify(body),
  });
  if (!res.ok) throw new Error(`${path}: ${res.status} ${await res.text()}`);
  return res.json();
}

const navigate = (id, url) =>
  api(`/browsers/${id}/command`, { action: 'navigate', params: { url } }).catch((err) =>
    console.warn(`navigate ${id}:`, err.message),
  );

/** Waits for a background run to stop, whichever way it stops. */
async function settled(runId) {
  for (let i = 0; i < RUN_POLL_TRIES; i++) {
    const run = await api(`/runs/${runId}`);
    if (run.status !== 'running') return run;
    await sleep(RUN_POLL_MS);
  }
  return { status: 'timeout' };
}

/**
 * Makes sure PLAYBOOK exists before the camera rolls.
 *
 * The recording pass has a model in it and takes about half a minute; the GIF window is 22
 * seconds. So the demonstration films the replay, which is the claim anyway, and the run
 * that produced the playbook happens here, off camera, only if it is not saved already.
 */
async function ensurePlaybook(browserId) {
  const saved = await api('/playbooks')
    .then((r) => r.playbooks || [])
    .catch(() => []);
  const found = saved.find((p) => p.name === PLAYBOOK);
  if (found) return found;
  console.log(`Recording ${PLAYBOOK} once, with a model, off camera...`);
  await navigate(browserId, DEMO_SITE);
  const started = await api(`/browsers/${browserId}/runs`, { prompt: DEMO_TASK, data: DEMO_DATA });
  const run = await settled(started.runId || started.id);
  if (run.status !== 'succeeded') throw new Error(`the demo task did not finish: ${run.status} ${run.error || ''}`);
  return api(`/browsers/${browserId}/playbooks`, { name: PLAYBOOK });
}

async function record() {
  // Healthy ones only: a session whose browser has gone leaves a dead row behind, and it
  // sorts no differently from a live one.
  const fleet = (await api('/browsers')).filter((b) => b.health === 'ok');
  if (fleet.length < 2) throw new Error(`Only ${fleet.length} healthy browser(s) on ${BASE}; start at least two`);
  console.log(`Recording ${fleet.length} real browsers on ${BASE}`);
  // A fresh key opens on first-run onboarding; skip it so the console shows the fleet.
  await api('/config', { onboarded: 'true', desktop_seen_at: new Date().toISOString() });
  // Start every session under agent control so the takeover scene begins clean.
  for (const b of fleet) {
    const { control } = await api(`/control/sessions/${b.id}`).catch(() => ({ control: { mode: 'agent' } }));
    if (control.mode === 'human') await api(`/control/sessions/${b.id}/control`, { action: 'release' });
    if (control.mode !== 'agent') await api(`/control/sessions/${b.id}/control`, { action: 'resume' });
  }
  await Promise.all(fleet.map((b, i) => navigate(b.id, SITES[i % SITES.length])));

  // The browser the demonstration drives, and the playbook it replays. Both are settled before
  // the camera rolls, so what is filmed is the replay rather than the wait for a model.
  const star = fleet[0];
  const playbook = await ensurePlaybook(star.id);
  const steps = playbook.steps ?? playbook.playbook?.steps?.length ?? '';
  console.log(`Filming ${PLAYBOOK} (${steps} steps) on ${star.id}`);

  rmSync(TEMP_VIDEO_DIR, { recursive: true, force: true });
  mkdirSync(TEMP_VIDEO_DIR, { recursive: true });

  const browser = await chromium.launch({ channel: 'chrome', headless: true });
  const context = await browser.newContext({
    viewport: { width: 1920, height: 1080 },
    recordVideo: { dir: TEMP_VIDEO_DIR, size: { width: 1920, height: 1080 } },
  });
  const page = await context.newPage();

  // Presentation only: the API key, a visible cursor and a chapter caption.
  await context.addInitScript((key) => {
    // The console reads its credential from sessionStorage as `oya_console_key`
    // (ui/src/lib/api.ts). `oya_api_key` is the pre-session name, which the console now only
    // ever clears, setting just that one is why this script stopped being able to film
    // anything, and why the committed assets are older than the features they show.
    sessionStorage.setItem('oya_console_key', key);
    localStorage.setItem('oya_api_key', key);
    window.addEventListener('DOMContentLoaded', () => {
      const style = document.createElement('style');
      style.textContent = `
        nextjs-portal, [data-nextjs-toast] { display: none !important; }
        #studio-cursor { position: fixed; width: 14px; height: 14px; border-radius: 50%; background: #39ed35;
          border: 1.5px solid #0c0c0a; box-shadow: 0 0 12px rgba(57,237,53,.9); pointer-events: none;
          z-index: 9999999; transform: translate(-50%, -50%); }
        .click-ripple { position: fixed; width: 14px; height: 14px; border-radius: 50%; border: 2px solid #39ed35;
          pointer-events: none; z-index: 9999998; animation: ripple .45s ease-out forwards; }
        @keyframes ripple { from { transform: translate(-50%,-50%) scale(1); opacity: 1 }
          to { transform: translate(-50%,-50%) scale(4.5); opacity: 0 } }
        #chapter-hud { position: fixed; bottom: 24px; left: 50%; transform: translateX(-50%); z-index: 9999990;
          background: rgba(18,18,14,.92); border: 1px solid rgba(57,237,53,.4); border-radius: 24px;
          padding: 9px 22px; color: #fff; font: 600 13px -apple-system, BlinkMacSystemFont, sans-serif;
          letter-spacing: .5px; box-shadow: 0 10px 36px rgba(0,0,0,.7); }
      `;
      document.head.appendChild(style);
      const cursor = Object.assign(document.createElement('div'), { id: 'studio-cursor' });
      const hud = Object.assign(document.createElement('div'), { id: 'chapter-hud' });
      document.body.append(cursor, hud);
      window.addEventListener('mousemove', (e) => {
        cursor.style.left = `${e.clientX}px`;
        cursor.style.top = `${e.clientY}px`;
      });
      window.addEventListener('mousedown', (e) => {
        const r = Object.assign(document.createElement('div'), { className: 'click-ripple' });
        r.style.left = `${e.clientX}px`;
        r.style.top = `${e.clientY}px`;
        document.body.appendChild(r);
        setTimeout(() => r.remove(), 450);
      });
    });
    window.__setHud = (text) => {
      const el = document.getElementById('chapter-hud');
      if (el) el.textContent = text;
    };
  }, KEY);

  const hud = (text) => page.evaluate((t) => window.__setHud(t), text);
  async function clickTo(locator, pause = 1500) {
    if (!(await locator.isVisible().catch(() => false))) return false;
    const box = await locator.boundingBox();
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2, { steps: 22 });
    await locator.click();
    await sleep(pause);
    return true;
  }

  // The live view is an open SSE stream, so wait for rows, not network idle.
  await page.goto(`${BASE}/dashboard`);
  await page.locator('tbody tr').first().waitFor({ timeout: 30_000 });
  // By name, because the row that sorts first is not necessarily the one being driven.
  const starRow = page.locator('tbody tr', { hasText: star.name }).first();

  await hud(`01 / PLAYBOOK, one saved run, ${steps} steps, no model needed`);
  await clickTo(page.getByRole('button', { name: 'Playbooks', exact: true }), 2500);
  await page.mouse.move(520, 300, { steps: 20 });
  await sleep(1500);

  // The claim, filmed in real time rather than sped up: the replay drives the browser with
  // no model in the loop. Start it, then watch it happen in the live view.
  await hud('02 / REPLAY, same task, new inputs, no model in the loop');
  await navigate(star.id, DEMO_SITE);
  const replay = api(`/browsers/${star.id}/playbooks/${PLAYBOOK}/play`, { data: DEMO_DATA, autoHeal: false }).catch(
    (err) => console.warn('replay:', err.message),
  );
  await clickTo(page.getByRole('button', { name: 'Browsers', exact: true }), 800);
  await clickTo(starRow, 1200);
  await sleep(REPLAY_WATCH_MS);
  await replay;
  await sleep(1200);

  await hud('03 / PLAYWRIGHT, the code it wrote, yours to keep');
  await clickTo(page.getByRole('button', { name: 'Playbooks', exact: true }), 1200);
  await clickTo(page.getByText(PLAYBOOK).first(), 2500);

  // Kept from the original tour, because a run that stops for a person is the other half
  // of the story and nobody else films it.
  await hud('04 / WHEN IT NEEDS A PERSON, take the mouse, then hand it back');
  await clickTo(page.getByRole('button', { name: 'Browsers', exact: true }), 800);
  await clickTo(starRow, 1200);
  if (await clickTo(page.getByRole('button', { name: 'Take control' }), 1500)) {
    await clickTo(page.getByRole('button', { name: 'Release control' }), 1200);
    await clickTo(page.getByRole('button', { name: 'Resume agent' }), 1200);
  }

  await hud('05 / AND THE RECEIPTS, sessions, audit trail, spend');
  await clickTo(page.getByRole('button', { name: 'Control', exact: true }), 2500);

  await hud('Oya Browser, npm i @oya-ai/browser');
  await page.mouse.move(960, 500, { steps: 25 });
  await sleep(2000);

  // The poster is a still, so take it as one, with the chapter caption cleared. Grabbing a
  // video frame instead is why the old poster had "04 / DRIVE …" burned across the bottom.
  await hud('');
  await clickTo(page.getByRole('button', { name: 'Playbooks', exact: true }), 1500);
  await page.screenshot({ path: join(UI_DIR, 'public', 'oya-browser-poster.jpg'), quality: 88, type: 'jpeg' });
  await page.screenshot({ path: join(ROOT_DIR, 'assets', 'oya-console-overview.png') });

  await context.close();
  await browser.close();

  const webm = readdirSync(TEMP_VIDEO_DIR).find((f) => f.endsWith('.webm'));
  if (!webm) throw new Error(`No recording in ${TEMP_VIDEO_DIR}`);
  const raw = join(TEMP_VIDEO_DIR, webm);
  const mp4 = join(UI_DIR, 'public', 'oya-browser.mp4');

  console.log('Transcoding MP4 and the GIF...');
  await run(
    `ffmpeg -y -loglevel error -i "${raw}" -vf "fps=30" -c:v libx264 -preset slow -crf 20 -pix_fmt yuv420p -movflags +faststart "${mp4}"`,
  );
  // The GIF is the argument, so it covers the replay: chapters 01 and 02, from the saved
  // playbook to the browser driving itself with no model. Real time, not sped up.
  await run(
    `ffmpeg -y -loglevel error -ss 1 -t 26 -i "${mp4}" -vf "fps=10,scale=960:540:flags=lanczos,split[a][b];[a]palettegen=max_colors=128[p];[b][p]paletteuse=dither=bayer:bayer_scale=3" "${join(ROOT_DIR, 'assets', 'oya-replay-demo.gif')}"`,
  );

  rmSync(TEMP_VIDEO_DIR, { recursive: true, force: true });
  console.log('Done.');
}

function run(cmd) {
  return new Promise((resolve, reject) => {
    spawn(cmd, { shell: true, stdio: 'inherit' }).on('close', (code) =>
      code === 0 ? resolve() : reject(new Error(`${cmd} exited ${code}`)),
    );
  });
}

record().catch((err) => {
  console.error('Recording failed:', err);
  process.exit(1);
});
