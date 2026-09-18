/**
 * The prompt layer, driven both ways.
 *
 * The terminal path and the piped path are different code, and only one of them
 * is exercised by running the wizard in CI — so the arrow-key menu is driven here
 * with synthetic key events against a faked TTY. The behaviour that matters most
 * is that bad input re-prompts: a wizard that exits on a typo throws away every
 * answer given so far.
 *
 * Imports the TypeScript directly — Node strips the types.
 */

import assert from 'node:assert/strict';
import { stdin, stdout } from 'node:process';
import { spawnSync } from 'node:child_process';

const { choose, InputError } = await import('../../src/prompt.ts');

const ESC = '\u001b';
const OPTIONS = [
  { id: 'docker', label: 'Docker on this machine' },
  { id: 'k8s', label: 'Kubernetes', disabled: 'not in this build yet' },
  { id: 'ecs', label: 'Amazon ECS', disabled: 'not in this build yet' },
  { id: 'cdp', label: 'Your own Chrome' },
];

/** Swallow the drawing so assertions read against captured text, not the terminal. */
function captureStdout() {
  const chunks = [];
  const real = stdout.write.bind(stdout);
  stdout.write = (chunk, ...rest) => {
    chunks.push(String(chunk));
    return real('', ...rest.filter(() => false)) || true;
  };
  return {
    text: () => chunks.join(''),
    restore: () => {
      stdout.write = real;
    },
  };
}

/** Pretend stdin is a terminal, and deliver keystrokes as readline would. */
async function withFakeTty(keys, run) {
  const realIsTTY = Object.getOwnPropertyDescriptor(stdin, 'isTTY');
  const realSetRaw = stdin.setRawMode;
  const realResume = stdin.resume;
  Object.defineProperty(stdin, 'isTTY', { value: true, configurable: true });
  stdin.setRawMode = () => stdin;
  stdin.resume = () => stdin;

  const cap = captureStdout();
  try {
    const promise = run();
    // The menu subscribes on 'data' once it has drawn itself.
    for (const key of keys) {
      await new Promise((r) => setImmediate(r));
      stdin.emit('data', Buffer.from(key));
    }
    return { value: await promise, output: cap.text() };
  } finally {
    cap.restore();
    stdin.setRawMode = realSetRaw;
    stdin.resume = realResume;
    if (realIsTTY) Object.defineProperty(stdin, 'isTTY', realIsTTY);
    else delete stdin.isTTY;
  }
}

let passed = 0;
const ok = (label) => {
  passed++;
  console.log(`  ✅ ${label}`);
};

// ── Arrow keys move, and enter selects ──────────────────────────────────────
{
  const { value } = await withFakeTty(['\r'], () => choose('Where?', OPTIONS));
  assert.equal(value, 'docker', 'enter takes the first selectable option');
  ok('enter selects the highlighted option');
}

// ── Disabled entries are skipped, never landed on ───────────────────────────
{
  // One step down from 'docker' must skip both disabled rows and reach 'cdp'.
  const { value } = await withFakeTty([`${ESC}[B`, '\r'], () => choose('Where?', OPTIONS));
  assert.equal(value, 'cdp', 'down arrow skipped the two disabled options');
  ok('arrow movement steps over "not in this build yet" entries');
}

// ── Movement wraps ──────────────────────────────────────────────────────────
{
  const { value } = await withFakeTty([`${ESC}[A`, '\r'], () => choose('Where?', OPTIONS));
  assert.equal(value, 'cdp', 'up from the first selectable wraps to the last');
  ok('movement wraps around the list');
}

// ── A number key jumps, but not onto a disabled row ─────────────────────────
{
  const { value } = await withFakeTty(['2', '\r'], () => choose('Where?', OPTIONS));
  assert.equal(value, 'docker', 'pressing 2 (a disabled row) did not move the selection');
  ok('number keys refuse to jump to a disabled option');
}
{
  const { value } = await withFakeTty(['4', '\r'], () => choose('Where?', OPTIONS));
  assert.equal(value, 'cdp');
  ok('number keys jump to an enabled option');
}

// ── Ctrl+C cancels without leaving the terminal in raw mode ─────────────────
{
  await assert.rejects(
    withFakeTty(['\u0003'], () => choose('Where?', OPTIONS)),
    (err) => err instanceof InputError,
    'Ctrl+C rejects with InputError',
  );
  assert.notEqual(stdin.isRaw, true, 'raw mode is not left enabled after a cancel');
  ok('Ctrl+C cancels and restores the terminal');
}

// ── Disabled options are shown, not hidden ──────────────────────────────────
{
  const { output } = await withFakeTty(['\r'], () => choose('Where?', OPTIONS));
  assert.match(output, /Kubernetes/, 'a not-yet-supported option is still listed');
  assert.match(output, /not in this build yet/, 'and says why it cannot be picked');
  ok('unavailable options stay visible with a reason');
}

// ── The piped path ──────────────────────────────────────────────────────────
// In a child process with a real pipe, not synthetic events: under `npm test`
// this process's own stdin is already at EOF, so an in-process fake would read
// nothing and quietly pass for the wrong reason.

function pipedRun(script, input) {
  const child = spawnSync(process.execPath, ['--input-type=module', '-e', script], {
    input,
    encoding: 'utf8',
    cwd: new URL('../..', import.meta.url).pathname,
  });
  return `${child.stdout}${child.stderr}`;
}

{
  const out = pipedRun(
    `
    const { choose } = await import('./src/prompt.ts');
    const picked = await choose('Where?', ${JSON.stringify(OPTIONS)});
    console.log('PICKED:' + picked);
    process.exit(0);
  `,
    '9\nk8s\n4\n',
  );

  assert.match(out, /PICKED:cdp/, 'it kept asking until a valid answer arrived');
  assert.match(out, /Pick one of the numbers above/, 'an out-of-range answer is explained');
  assert.match(out, /Kubernetes is not in this build yet/, 'a disabled answer is explained');
  ok('off a terminal, invalid and disabled answers re-prompt rather than exit');
}

{
  const out = pipedRun(
    `
    const { ask } = await import('./src/prompt.ts');
    const value = await ask('URL:', '', {
      validate: (v) => { try { new URL(v); } catch { return 'That is not a URL'; } },
    });
    console.log('GOT:' + value);
    process.exit(0);
  `,
    'nope\nhttp://localhost:3100\n',
  );

  assert.match(out, /GOT:http:\/\/localhost:3100/);
  assert.match(out, /That is not a URL/);
  ok('a rejected answer is re-asked, and earlier answers survive');
}

console.log(`\n  ${passed} passed, 0 failed`);
process.exit(0);
