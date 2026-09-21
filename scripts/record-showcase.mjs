/**
 * The three-minute showcase: the desktop browser doing the things that stop other agents.
 *
 * Four beats, in this order, because each one answers the objection the last one raises:
 *   1. Hacker News, the mechanic, in the open: ask, then keep the run as a playbook.
 *   2. LinkedIn   , already signed in. No password is typed and none is in the prompt; the
 *                    session arrived with the persona when the browser connected.
 *   3. Amazon     , a site that blocks automation, driven at a human pace.
 *   4. Sauce Demo , a login the browser completes by itself from a sealed credential.
 *
 * It films the real window with ffmpeg rather than capturing through an API, because the app
 * draws each tab as a BrowserView and neither Playwright's screenshot nor Electron's
 * capturePage() includes one, both return the chrome with an empty hole where the page is.
 * The window is pinned on top for the same reason: the camera films the screen, so anything
 * above that rectangle would be filmed instead.
 *
 * Needs macOS Screen Recording permission for the terminal running it (System Settings →
 * Privacy & Security → Screen & System Audio Recording), or ffmpeg produces nothing.
 *
 *   OYA_RECORD_KEY=<key> node scripts/record-showcase.mjs
 *
 * Writes assets/oya-demo.mp4 (subtitles burned in), assets/oya-demo.srt and
 * assets/oya-demo.gif (a short excerpt for the README).
 *
 * LinkedIn shows whichever account the persona is signed in as. Watch it back before
 * publishing it.
 */

import { spawn } from 'node:child_process';
import { mkdtemp, rm, mkdir, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { _electron as electron } from '../ui/node_modules/playwright/index.mjs';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');
const BROWSER_DIR = join(ROOT, 'browser');
const KEY = process.env.OYA_RECORD_KEY;
if (!KEY) throw new Error('Set OYA_RECORD_KEY to an API key on the stack being recorded');
const BASE = (process.env.OYA_RECORD_URL || 'http://localhost:3100').replace(/\/$/, '');
const WS = process.env.OYA_RECORD_WS || `${BASE.replace(/^http/, 'ws')}/ws`;

/** Where the window sits while filming. 720p-ish, so the GIF stays readable at README width. */
const WINDOW = { x: 60, y: 60, width: 1280, height: 800 };
/** The avfoundation index of "Capture screen 0". */
const SCREEN = process.env.OYA_RECORD_SCREEN || '5';
/** An agent run on a real site, worst case. Amazon is the slow one: it answers each field
 *  check with its own server, and a run there took past ninety seconds while still working. */
const ASK_MS = 150_000;
/** The excerpt the README GIF is cut from: the first beat, which explains the rest. */
const GIF = { start: 6, length: 26 };

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

/** One authenticated call to the stack being filmed. */
async function api(path, body) {
  const res = await fetch(`${BASE}/api${path}`, {
    method: body ? 'POST' : 'GET',
    headers: { Authorization: `Bearer ${KEY}`, 'Content-Type': 'application/json' },
    body: body && JSON.stringify(body),
  });
  if (!res.ok) throw new Error(`${path}: ${res.status} ${await res.text()}`);
  return res.json();
}

/** Runs a command, resolving when it exits cleanly. */
const run = (cmd, args) =>
  new Promise((resolve, reject) => {
    const child = spawn(cmd, args, { stdio: 'inherit' });
    child.on('exit', (code) => (code === 0 ? resolve() : reject(new Error(`${cmd} exited ${code}`))));
  });

/** Films the screen, cropped to the window, until stop() is called. */
function startCapture(out, scale, b) {
  const even = (n) => Math.floor((n * scale) / 2) * 2;
  const ffmpeg = spawn(
    'ffmpeg',
    [
      '-y',
      '-loglevel',
      'error',
      '-f',
      'avfoundation',
      '-capture_cursor',
      '1',
      '-framerate',
      '25',
      '-pix_fmt',
      'bgr0',
      '-i',
      SCREEN,
      '-vf',
      `crop=${even(b.width)}:${even(b.height)}:${b.x * scale}:${b.y * scale}`,
      '-c:v',
      'libx264',
      '-preset',
      'ultrafast',
      '-pix_fmt',
      'yuv420p',
      out,
    ],
    { stdio: ['pipe', 'inherit', 'inherit'] },
  );
  return {
    stop: () =>
      new Promise((resolve) => {
        ffmpeg.on('exit', resolve);
        ffmpeg.stdin.write('q');
        setTimeout(() => ffmpeg.kill('SIGINT'), 3000);
      }),
  };
}

/** Launches the app on a throwaway profile and pins it where the camera is pointed. */
async function launchApp(profile) {
  const require_ = createRequire(`${BROWSER_DIR}/`);
  const app = await electron.launch({
    executablePath: require_(join(BROWSER_DIR, 'launch.cjs')).developmentExecutable(),
    args: [join(BROWSER_DIR, 'main.js')],
    cwd: BROWSER_DIR,
    env: { ...process.env, OYA_USER_DATA_DIR: profile, OYA_API_KEY: '', OYA_AUTO_CONNECT: 'false', OYA_SERVER_URL: '' },
  });
  const page = await app.firstWindow();
  await page.waitForFunction(() => typeof window.shellIcon === 'function', null, { timeout: 30_000 });
  const bounds = await app.evaluate(async ({ BrowserWindow, app: electronApp }, want) => {
    const win = BrowserWindow.getAllWindows()[0];
    win.setBounds(want);
    win.show();
    // Pinned above everything: focus() alone does not raise a window launched from a shell,
    // and the first recording filmed the terminal that was sitting on top of it.
    win.setAlwaysOnTop(true, 'screen-saver');
    win.moveTop();
    win.focus();
    electronApp.focus({ steal: true });
    await new Promise((r) => setTimeout(r, 600));
    return win.getBounds();
  }, WINDOW);
  const scale = await app.evaluate(({ screen }) => screen.getPrimaryDisplay().scaleFactor).catch(() => 1);
  return { app, page, bounds, scale: Number(process.env.OYA_RECORD_SCALE) || scale };
}

/** Signs the app in to the stack the way a person does on first run. */
async function connect(page) {
  await page.fill('#cfg-server', WS);
  await page.fill('#cfg-key', KEY);
  await page.click('#btn-connect');
  await page.waitForSelector('#btn-dev', { state: 'visible', timeout: 30_000 });
}

/** Opens the agent panel, once, on the Ask tab. */
async function openAsk(page) {
  if ((await page.getAttribute('#btn-dev', 'aria-expanded')) !== 'true') await page.click('#btn-dev');
  await page.click('[data-pane="chat"]');
  await page.waitForSelector('#chat-input', { state: 'visible', timeout: 15_000 });
}

/** Types a task into Ask and waits for the reply that offers to keep it. */
async function ask(page, task) {
  await page.click('#chat-input');
  await page.type('#chat-input', task, { delay: 24 });
  await page.keyboard.press('Enter');
  await page.waitForSelector('.chat-save-button', { timeout: ASK_MS });
  await sleep(2000);
  // The analyzer outlines every element it can act on and never takes the labels away, so by
  // the time an answer arrives the page is under hundreds of them. Reloading is what a person
  // would do, and the answer stays on the right while the page underneath comes back clean.
  await page.click('#btn-reload').catch(() => {});
  await sleep(2500);
}

/** Keeps the last run under `name`, and says so if the app refuses. */
async function keepAsPlaybook(page, name) {
  await page.click('.chat-save-button');
  await page.fill('.chat-save-name', name);
  await sleep(600);
  await page.click('.chat-save-confirm');
  await page.waitForFunction(
    () => {
      const box = document.querySelector('.chat-save');
      return box?.classList.contains('saved') || !!box?.querySelector('.chat-save-error')?.textContent;
    },
    null,
    { timeout: 25_000 },
  );
  const failure = await page.textContent('.chat-save-error').catch(() => '');
  if (failure) throw new Error(`could not keep the playbook: ${failure}`);
  await sleep(2000);
}

/** The browser this app connected as, so a run can be aimed at it. */
async function thisBrowser(page) {
  const name = await page.getAttribute('#cfg-name', 'value').catch(() => null);
  const fleet = await api('/browsers');
  const live = fleet.filter((b) => b.health === 'ok');
  return (name && live.find((b) => b.name === name)) || live.at(-1);
}

/** Starts a prompt run on the browser and waits it out; runs carry the sign-in checkpoint. */
async function runTask(browserId, prompt) {
  const started = await api(`/browsers/${browserId}/runs`, { prompt });
  const id = started.runId || started.id;
  for (let i = 0; i < 40; i++) {
    const run = await api(`/runs/${id}`);
    if (run.status !== 'running') return run;
    await sleep(5000);
  }
  return { status: 'timeout' };
}

/** Seconds since filming began, as an SRT timestamp. */
const stamp = (seconds) => {
  const whole = Math.max(0, Math.floor(seconds));
  const pad = (n, w = 2) => String(n).padStart(w, '0');
  return `${pad(Math.floor(whole / 3600))}:${pad(Math.floor(whole / 60) % 60)}:${pad(whole % 60)},000`;
};

/** The caption track, written from what each beat actually took. */
const subtitles = (marks) =>
  marks.map((m, i) => `${i + 1}\n${stamp(m.from)} --> ${stamp(m.to)}\n${m.text}\n`).join('\n');

async function record() {
  const profile = await mkdtemp(join(tmpdir(), 'oya-showcase-'));
  const raw = join(tmpdir(), `oya-showcase-${Date.now()}.mp4`);
  await mkdir(join(ROOT, 'assets'), { recursive: true });
  const { app, page, bounds, scale } = await launchApp(profile);
  console.log(`Filming ${bounds.width}x${bounds.height} at (${bounds.x},${bounds.y}), scale ${scale}`);
  const capture = startCapture(raw, scale, bounds);
  const began = Date.now();
  const marks = [];
  /** Runs one beat and records when its caption should be on screen. */
  const beat = async (text, body) => {
    const from = (Date.now() - began) / 1000;
    console.log(`  ${text}`);
    await body();
    marks.push({ text, from, to: (Date.now() - began) / 1000 });
  };

  try {
    await sleep(1500);
    await beat('Connect the browser to your workspace', async () => {
      await connect(page);
      await sleep(1500);
    });
    const browser = await thisBrowser(page);
    await openAsk(page);

    await beat('Ask it to do the task once', () =>
      ask(page, 'Open news.ycombinator.com, click into the top story’s comments, and tell me its title.'),
    );
    await beat('Keep the run. It replays later with no model in the loop', () =>
      keepAsPlaybook(page, 'hn-top-comments'),
    );
    await beat('LinkedIn, already signed in, no password typed, none in the prompt', () =>
      ask(
        page,
        'Open https://www.linkedin.com/search/results/people/?keywords=platform%20engineer and tell me how many results it shows.',
      ),
    );
    await beat('Amazon, which blocks most automation, at a human pace', () =>
      ask(
        page,
        'Search amazon.com for a mechanical keyboard and tell me the title of the first search result. Stay on the results page.',
      ),
    );
    await beat('A login it completes by itself, from a sealed credential', async () => {
      const result = await runTask(
        browser.id,
        'Go to https://www.saucedemo.com/ and tell me the first product listed.',
      );
      console.log('   signed in and answered:', String(result.result?.text || result.status).slice(0, 80));
      await sleep(3000);
    });
  } finally {
    await capture.stop();
    await app.close().catch(() => {});
    await rm(profile, { recursive: true, force: true });
  }

  const srt = join(ROOT, 'assets', 'oya-demo.srt');
  await writeFile(srt, subtitles(marks));
  console.log('Transcoding with subtitles...');
  const mp4 = join(ROOT, 'assets', 'oya-demo.mp4');
  // A subtitle track rather than pixels: burning captions in needs the `subtitles` filter,
  // which exists only in an ffmpeg built with libass, and the stock macOS builds are not.
  // mov_text needs nothing extra, and the .srt beside it opens in any player or editor.
  await run('ffmpeg', [
    '-y',
    '-loglevel',
    'error',
    '-i',
    raw,
    '-i',
    srt,
    '-c:v',
    'libx264',
    '-preset',
    'medium',
    '-crf',
    '23',
    '-pix_fmt',
    'yuv420p',
    '-c:s',
    'mov_text',
    '-metadata:s:s:0',
    'language=eng',
    '-movflags',
    '+faststart',
    mp4,
  ]);
  await run('ffmpeg', [
    '-y',
    '-loglevel',
    'error',
    '-ss',
    String(GIF.start + 4),
    '-i',
    mp4,
    '-frames:v',
    '1',
    '-q:v',
    '2',
    join(ROOT, 'assets', 'oya-demo-poster.jpg'),
  ]);
  await rm(raw, { force: true });
  console.log(`Done: ${mp4} (${Math.round(marks.at(-1)?.to || 0)}s)`);
}

await record();
