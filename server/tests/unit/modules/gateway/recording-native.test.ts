/** Native recording cadence is sequential and stop never publishes late frames. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { NativeRecording } from '../../../../src/modules/gateway/recording-native.ts';
import { NATIVE_RECORD_INTERVAL_MS } from '../../../../src/modules/gateway/constants.ts';

test('slow capture never overlaps and stop discards its late result', async (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  let finish: (data: string) => void;
  let calls = 0;
  const frames: string[] = [];
  const recorder = new NativeRecording(async () => {
    calls++;
    if (calls === 1) return 'first';
    return new Promise<string>((resolve) => {
      finish = resolve;
    });
  });
  await recorder.start((data) => frames.push(data), assert.fail);
  t.mock.timers.tick(NATIVE_RECORD_INTERVAL_MS * 5);
  assert.equal(calls, 2);
  const stopped = recorder.stop();
  finish!('late');
  await stopped;
  t.mock.timers.tick(NATIVE_RECORD_INTERVAL_MS * 5);
  assert.deepEqual(frames, ['first']);
  assert.equal(calls, 2);
});
test('initial capture failure rejects startup', async () => {
  const recorder = new NativeRecording(async () => {
    throw Error('control denied');
  });
  await assert.rejects(recorder.start(assert.fail, assert.fail), /control denied/);
  await recorder.stop();
});
test('later capture failure is reported once and ends sampling', async (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  let calls = 0;
  const failures: unknown[] = [];
  const recorder = new NativeRecording(async () => {
    if (++calls > 1) throw Error('disconnected');
    return 'first';
  });
  await recorder.start(
    () => {},
    (error) => failures.push(error),
  );
  t.mock.timers.tick(NATIVE_RECORD_INTERVAL_MS);
  await recorder.stop();
  t.mock.timers.tick(NATIVE_RECORD_INTERVAL_MS * 5);
  assert.equal(calls, 2);
  assert.equal(failures.length, 1);
});

test('native fleet recording bypasses protocol endpoints and seals its manifest after writes finish', async (t) => {
  const { scriptedOyaBrowser } = await import('../../support/agent.ts');
  const { stubControl } = await import('../../support/browsers.ts');
  const { randomUUID } = await import('node:crypto');
  const { readFile } = await import('node:fs/promises');
  const { join } = await import('node:path');
  const live = await import('../../../../src/modules/gateway/recording-live.ts');
  const { openManifest, openFrame } = await import('../../../../src/modules/control/recording-storage.ts');
  stubControl();
  const browser = scriptedOyaBrowser('native-recording', 'owner-key', (action) => ({
    ok: true,
    data:
      action === 'list_tabs'
        ? { tabs: [{ id: 7, url: 'https://example.test' }] }
        : { screenshot: 'data:image/jpeg;base64,anBlZw==' },
  }));
  t.after(() => {
    browser.disconnect();
    t.mock.restoreAll();
  });
  const session = {
    id: randomUUID(),
    attachedTo: 'native-recording',
    apiKey: 'owner-key',
    owner: 'owner-digest',
    provider: 'oya',
    profile: null,
    endpoint: {
      open() {
        assert.fail('native recording opened protocol');
      },
    },
  };
  await live.start(session);
  await live.stop(session.id);
  assert.deepEqual(
    browser.calls.map(({ action }) => action),
    ['list_tabs', 'screenshot'],
  );
  assert.deepEqual(browser.calls[1].params, { tab_id: 7, format: 'jpeg' });
  const manifest = openManifest(session.id, await readFile(join(live.DIR, session.id, 'manifest.json')));
  assert.equal(manifest.owner, 'owner-digest');
  assert.equal(manifest.frameCount, 1);
  const bytes = await readFile(join(live.DIR, session.id, live.frameFile(0)));
  assert.equal(openFrame(session.id, 0, bytes).toString(), 'jpeg');
  assert.equal(bytes.includes('jpeg'), false);
});

test('missing Oya ownership never falls back to a supplied protocol endpoint', async () => {
  const { nativeRecording } = await import('../../../../src/modules/gateway/recording-native.ts');
  for (const provider of ['oya', 'oya-cloud', 'oya-selfhosted'])
    await assert.rejects(nativeRecording({ provider, endpoint: { url: 'ws://127.0.0.1:9222' } }), /unavailable/);
});
