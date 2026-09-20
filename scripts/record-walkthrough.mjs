/**
 * Records the README / landing-page walkthrough against a REAL running Oya
 * stack: real browsers, real live view, real personas. Nothing is mocked.
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
 * assets/oya-fleet-demo.gif and assets/oya-console-overview.png.
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

const SITES = ['https://news.ycombinator.com', 'https://en.wikipedia.org/wiki/Web_browser', 'https://github.com/trending'];

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

const navigate = (id, url) => api(`/browsers/${id}/command`, { action: 'navigate', params: { url } })
  .catch((err) => console.warn(`navigate ${id}:`, err.message));

async function record() {
  const fleet = await api('/browsers');
  if (fleet.length < 2) throw new Error(`Only ${fleet.length} browser(s) connected to ${BASE}; start at least two`);
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
      window.addEventListener('mousemove', (e) => { cursor.style.left = `${e.clientX}px`; cursor.style.top = `${e.clientY}px`; });
      window.addEventListener('mousedown', (e) => {
        const r = Object.assign(document.createElement('div'), { className: 'click-ripple' });
        r.style.left = `${e.clientX}px`; r.style.top = `${e.clientY}px`;
        document.body.appendChild(r);
        setTimeout(() => r.remove(), 450);
      });
    });
    window.__setHud = (text) => { const el = document.getElementById('chapter-hud'); if (el) el.textContent = text; };
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

  await hud(`01 / FLEET — ${fleet.length} real browsers, one console`);
  await page.mouse.move(500, 250, { steps: 20 });
  await sleep(1200);
  await page.mouse.move(190, 122, { steps: 20 });
  await sleep(1200);

  await hud('02 / LIVE VIEW — the actual page, streamed');
  await clickTo(page.locator('tbody tr').first(), 3000);

  // Console input (navigate, elements) is human input: it only runs while a person holds control.
  await hud('03 / TAKEOVER — a human takes control of the browser');
  if (await clickTo(page.getByRole('button', { name: 'Take control' }), 1500)) {
    await hud('04 / DRIVE — and navigates it from the console');
    const address = page.getByLabel('Navigate to URL');
    if (await clickTo(address, 300)) {
      await page.keyboard.press('Meta+A');
      await page.keyboard.type('https://en.wikipedia.org/wiki/Headless_browser', { delay: 40 });
      await page.keyboard.press('Enter');
      await sleep(4500);
    }
    await hud('05 / ELEMENTS — numbered IDs an agent acts on');
    await clickTo(page.getByRole('button', { name: /Elements/ }), 2500);
    await hud('06 / HAND BACK — release control, the agent resumes');
    await clickTo(page.getByRole('button', { name: 'Release control' }), 1500);
    await clickTo(page.getByRole('button', { name: 'Resume agent' }), 1500);
  }

  await hud('07 / PERSONAS — fingerprint, cookies and proxy, fixed for life');
  await clickTo(page.getByRole('button', { name: 'Profiles', exact: true }), 2500);

  await hud('08 / CONTROL — sessions, audit log, spend');
  await clickTo(page.getByRole('button', { name: 'Control', exact: true }), 2500);

  await hud('Oya Browser — npm i @oya-ai/browser');
  await page.mouse.move(960, 500, { steps: 25 });
  await sleep(2200);

  await context.close();
  await browser.close();

  const webm = readdirSync(TEMP_VIDEO_DIR).find((f) => f.endsWith('.webm'));
  if (!webm) throw new Error(`No recording in ${TEMP_VIDEO_DIR}`);
  const raw = join(TEMP_VIDEO_DIR, webm);
  const mp4 = join(UI_DIR, 'public', 'oya-browser.mp4');

  console.log('Transcoding MP4, poster, screenshot and GIF...');
  await run(`ffmpeg -y -loglevel error -i "${raw}" -vf "fps=30" -c:v libx264 -preset slow -crf 20 -pix_fmt yuv420p -movflags +faststart "${mp4}"`);
  await run(`ffmpeg -y -loglevel error -ss 15 -i "${mp4}" -frames:v 1 -q:v 2 "${join(UI_DIR, 'public', 'oya-browser-poster.jpg')}"`);
  await run(`ffmpeg -y -loglevel error -ss 15 -i "${mp4}" -frames:v 1 "${join(ROOT_DIR, 'assets', 'oya-console-overview.png')}"`);
  await run(`ffmpeg -y -loglevel error -ss 2 -t 22 -i "${mp4}" -vf "fps=10,scale=960:540:flags=lanczos,split[a][b];[a]palettegen=max_colors=128[p];[b][p]paletteuse=dither=bayer:bayer_scale=3" "${join(ROOT_DIR, 'assets', 'oya-fleet-demo.gif')}"`);

  rmSync(TEMP_VIDEO_DIR, { recursive: true, force: true });
  console.log('Done.');
}

function run(cmd) {
  return new Promise((resolve, reject) => {
    spawn(cmd, { shell: true, stdio: 'inherit' })
      .on('close', (code) => (code === 0 ? resolve() : reject(new Error(`${cmd} exited ${code}`))));
  });
}

record().catch((err) => {
  console.error('Recording failed:', err);
  process.exit(1);
});
