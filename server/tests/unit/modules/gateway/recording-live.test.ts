/**
 * Unit tests for live recording: the screencast of a loopback browser's page
 * spooled frame by frame, acknowledged, capped, and closed with a manifest.
 * The frame cap is lowered to 2 for this file.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { readFileSync, existsSync } from 'node:fs';
import { join } from 'node:path';
import { ownDataDir, restoreEnv } from '../../support/data-dir.ts';

const dir = ownDataDir('oya-recording-live-');
process.env.OYA_RECORD_MAX_FRAMES = '2';
const live = await import('../../../../src/modules/gateway/recording-live.ts');
const { control } = await import('../../../../src/modules/control/service.ts');
const { fakeCdp, pageBrowser } = await import('../../support/gateway.ts');

const JPEG = Buffer.from('fake-jpeg').toString('base64');
const opened: { close(): Promise<void> }[] = [];
afterEach(async () => {
  await Promise.all(opened.splice(0).map((b) => b.close()));
  mock.restoreAll();
});

/** A session over a loopback browser with one page. */
async function recordable(extra: object = {}) {
  const browser = await fakeCdp(pageBrowser());
  opened.push(browser);
  return {
    browser,
    session: {
      id: randomUUID(),
      upstreamUrl: browser.url,
      provider: 'chrome',
      owner: 'abcdef12',
      profile: null,
      ...extra,
    },
  };
}
/** Waits until `check` holds, polling briefly. */
async function until(check: () => boolean) {
  for (let i = 0; i < 1000 && !check(); i++) await new Promise((r) => setTimeout(r, 5));
  assert.ok(check(), 'condition never held');
}

describe('recording-live', () => {
  it('names frame files by zero-padded index', () => {
    assert.equal(live.frameFile(42), '000042.jpg');
    assert.equal(live.DIR, join(dir, 'recordings'));
  });

  it('starts the screencast on the page and reports it as recording', async () => {
    const { browser, session } = await recordable();
    assert.equal(await live.start(session), true);
    assert.equal(live.isRecording(session.id), true);
    const cast = browser.commands.find((c) => c.method === 'Page.startScreencast');
    assert.equal(cast.sessionId, 's-1');
    assert.equal(cast.params.format, 'jpeg');
    await live.stop(session.id);
  });

  it('refuses to start twice for one session', async () => {
    const { session } = await recordable();
    await live.start(session);
    assert.equal(await live.start(session), false);
    await live.stop(session.id);
  });

  it('does not start without a page', async () => {
    const browser = await fakeCdp(() => ({ targetInfos: [] }));
    opened.push(browser);
    assert.equal(await live.start({ id: randomUUID(), upstreamUrl: browser.url }), false);
  });

  it('acknowledges and spools each frame, stopping at the frame cap', async () => {
    const { browser, session } = await recordable();
    await live.start(session);
    for (let i = 0; i < 3; i++)
      browser.emit(
        'Page.screencastFrame',
        { sessionId: 7 + i, data: JPEG, metadata: { deviceWidth: 800, deviceHeight: 600 } },
        's-1',
      );
    await until(() => browser.commands.filter((c) => c.method === 'Page.screencastFrameAck').length === 3);
    await until(() => existsSync(join(live.DIR, session.id, live.frameFile(1))));
    assert.equal(live.active.get(session.id).frames.length, 2);
    assert.equal(readFileSync(join(live.DIR, session.id, live.frameFile(0))).toString(), 'fake-jpeg');
    await live.stop(session.id);
  });

  it('writes a manifest on stop, owned and marked truncated when capped', async () => {
    const { browser, session } = await recordable({ profile: 'shop' });
    await live.start(session);
    for (let i = 0; i < 2; i++) browser.emit('Page.screencastFrame', { sessionId: i, data: JPEG }, 's-1');
    await until(() => live.active.get(session.id).frames.length === 2);
    assert.equal(await live.stop(session.id), true);
    const m = JSON.parse(readFileSync(join(live.DIR, session.id, 'manifest.json'), 'utf8'));
    assert.deepEqual(
      [m.sessionId, m.owner, m.provider, m.profile, m.frameCount, m.truncated, m.frames[0].meta],
      [session.id, 'abcdef12', 'chrome', 'shop', 2, true, null],
    );
    assert.equal(live.isRecording(session.id), false);
    assert.ok(browser.commands.some((c) => c.method === 'Page.stopScreencast'));
  });

  it('says false when asked to stop a session it is not recording', async () => {
    assert.equal(await live.stop(randomUUID()), false);
  });

  it('refuses with a 422 a session whose policy redacts recordings', async () => {
    const { session } = await recordable();
    mock.method(control().store as any, 'get', async (kind) =>
      kind === 'session' ? { policies: [{ redactRecording: true }] } : null,
    );
    await assert.rejects(live.start(session), {
      status: 422,
      message: 'Recording is disabled by the visual-redaction policy',
    });
  });

  it("refuses with a 422 a session whose project's policy redacts recordings", async () => {
    const { session } = await recordable();
    mock.method(control().store as any, 'get', async (kind) =>
      kind === 'session' ? { project: 'pr-1', policies: [] } : { settings: { policy: { redactRecording: true } } },
    );
    await assert.rejects(live.start(session), { status: 422 });
  });

  it('refuses with a 422 across replicas without shared recording storage', async () => {
    const { session } = await recordable();
    const saved = process.env.OYA_INSTANCE_URL;
    process.env.OYA_INSTANCE_URL = 'http://replica-1.internal';
    await assert.rejects(live.start(session), {
      status: 422,
      message: 'Distributed recording requires OYA_RECORDING_BUCKET',
    });
    restoreEnv('OYA_INSTANCE_URL', saved);
  });
});
