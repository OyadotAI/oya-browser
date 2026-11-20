/**
 * Real Electron check of the install line's last step: an oya://agent-connect link
 * that reaches the running app the way macOS delivers it (open-url) raises a dialog
 * of the app's own window, Cancel changes nothing, and an agent the app does not
 * know is refused. The dialog is answered by replacing showMessageBox before the
 * link is sent. HOME is left alone, because macOS looks for the keychain there, so
 * this check never connects an agent: that would write the person's real config.
 * Connecting itself is the footer's code path, covered by local-mcp/agents.test.ts.
 * Run: npm run test:connect-link (macOS).
 */
import assert from 'node:assert/strict';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { _electron as electron } from '../../../ui/node_modules/playwright/index.mjs';

/** How long the app has to show a dialog: it waits for its MCP port first. */
const DIALOG_WAIT_MS = 30_000;
/** How often the check looks for the dialog. */
const POLL_MS = 250;
/** A port of its own: the person's own Oya may hold the usual one, and then this copy offers none. */
const SPARE_PORT = 47_399;
/** The pause the app keeps after one link before it answers another. */
const LINK_COOLDOWN_MS = 10_000;

/** Sends `link` as macOS would and answers the consent dialog with Cancel. */
function sendLink(app, link) {
  return app.evaluate(({ app: electronApp }, url) => electronApp.emit('open-url', { preventDefault() {} }, url), link);
}

/** The dialogs the app has shown, once at least `count` have appeared or the wait runs out. */
async function dialogs(app, count) {
  const deadline = Date.now() + DIALOG_WAIT_MS;
  for (;;) {
    const asked = await app.evaluate(() => globalThis.asked);
    if (asked.length >= count || Date.now() > deadline) return asked;
    await new Promise((resolve) => setTimeout(resolve, POLL_MS));
  }
}

const profile = await mkdtemp(join(tmpdir(), 'oya-connect-link-'));
let app;
try {
  const root = fileURLToPath(new URL('../../', import.meta.url));
  app = await electron.launch({
    executablePath: createRequire(import.meta.url)('electron'),
    args: [root],
    cwd: root,
    env: {
      ...process.env,
      OYA_USER_DATA_DIR: profile,
      OYA_API_KEY: '',
      OYA_AUTO_CONNECT: 'false',
      OYA_MCP_PORT: String(SPARE_PORT),
    },
  });
  await app.firstWindow();
  // Every dialog is recorded and cancelled, before any link is sent, so none reaches the screen.
  await app.evaluate(({ dialog }) => {
    globalThis.asked = [];
    dialog.showMessageBox = async (...args) => {
      globalThis.asked.push(args.find((a) => a && a.message));
      return { response: 0, checkboxChecked: false };
    };
  });

  await sendLink(app, 'oya://agent-connect?agent=nope');
  const unknown = await dialogs(app, 1);
  assert.equal(unknown[0]?.message, 'This link names an agent Oya does not know');
  assert.equal(unknown.length, 1, 'nothing is asked for an unknown agent');

  await new Promise((resolve) => setTimeout(resolve, LINK_COOLDOWN_MS + POLL_MS));
  await sendLink(app, 'oya://agent-connect?agent=claude-code&token=evil');
  const asked = await dialogs(app, 2);
  const second = asked[1];
  if (second?.message === 'Connect Claude Code to Oya Browser?') {
    assert.deepEqual(second.buttons, ['Cancel', 'Connect Claude Code']);
    assert.equal(second.defaultId, 0, 'Return cancels');
    assert.equal(second.cancelId, 0, 'Escape cancels');
    assert.ok(!JSON.stringify(second).includes('evil'), 'nothing from the link is shown');
  } else {
    // Claude Code already lists this copy, or is not installed: the app says so and asks nothing.
    assert.match(second?.message ?? '', /Claude Code is (already connected|not installed on this computer)/);
  }
  assert.equal(asked.length, 2, 'Cancel shows nothing further and connects nothing');
  console.log(
    'Agent connect link passed: open-url raises the dialog, Cancel is the default and changes nothing, unknown agent refused.',
  );
} finally {
  await app?.close();
  await rm(profile, { recursive: true, force: true });
}
