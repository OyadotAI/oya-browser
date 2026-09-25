/**
 * Unit tests for the browser registry: registering and removing browsers,
 * per-key scoping, the activity log and derived health, and live-view viewers.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { registry, summarise } from '../../../../src/modules/browsers/registry.ts';
import {
  ACTIVITY_SIZE,
  DEAD_AFTER_MS,
  ERROR_CHARS,
  FAILING_COMMANDS,
  STALE_AFTER_MS,
  VIEWER_BACKLOG_BYTES,
} from '../../../../src/modules/browsers/constants.ts';
import { connectBrowser, disconnectBrowser } from '../../support/fakes.ts';
import { FakeResponse, driveBrowser } from '../../support/browsers.ts';

const B = 'b-registry';

/** Records one settled command on B. */
const settle = (ok: boolean, extra: object = {}) => registry.recordActivity(B, { action: 'click', ok, ...extra });

describe('registry: membership', () => {
  afterEach(() => {
    disconnectBrowser(B);
    disconnectBrowser('b-other');
    mock.restoreAll();
  });

  it('announces a browser when it is added and when it is removed', () => {
    const events = [];
    const onConnect = (e) => events.push(['connected', e.id, e.clientType]);
    const onDisconnect = (e) => events.push(['disconnected', e.id]);
    registry.on('browser:connected', onConnect);
    registry.on('browser:disconnected', onDisconnect);
    connectBrowser(B);
    disconnectBrowser(B);
    registry.off('browser:connected', onConnect);
    registry.off('browser:disconnected', onDisconnect);
    assert.deepEqual(events, [
      ['connected', B, 'oya'],
      ['disconnected', B],
    ]);
  });

  it('tells only the key’s own open Oya browsers, and survives a socket that fails', () => {
    const mine = connectBrowser(B, 'key-a');
    const theirs = connectBrowser('b-other', 'key-b');
    assert.equal(registry.tell('key-a', { type: 'settings_changed' }), 1);
    assert.deepEqual(mine.sent.at(-1), { type: 'settings_changed' });
    assert.equal(theirs.sent.length, 0);
    mine.failWith = new Error('closed');
    assert.equal(registry.tell('key-a', { type: 'settings_changed' }), 0);
  });

  it('ignores removing a browser it does not hold', () => {
    assert.doesNotThrow(() => registry.remove('never-added'));
  });

  it('belongs only to the key that connected it', () => {
    connectBrowser(B, 'key-a');
    assert.equal(registry.belongsTo(B, 'key-a'), true);
    assert.equal(registry.belongsTo(B, 'key-b'), false);
    assert.equal(registry.belongsTo('missing', 'key-a'), false);
  });

  it('lists only the caller’s browsers, or every browser without a key', () => {
    connectBrowser(B, 'key-a');
    connectBrowser('b-other', 'key-b');
    assert.deepEqual(
      registry.list('key-a').map((r) => r.id),
      [B],
    );
    assert.ok(registry.list().length >= 2);
  });

  it('describes a browser with its identity, counters and activity', () => {
    connectBrowser(B);
    registry.updateUrl(B, 'https://example.com');
    const d = registry.describe(B);
    assert.equal(d.name, 'Test');
    assert.equal(d.persona, 'p-1');
    assert.equal(d.currentUrl, 'https://example.com');
    assert.equal(d.health, 'ok');
    assert.deepEqual(d.activity, []);
    assert.equal(registry.describe('missing'), null);
  });

  it('fills in a name and key when the spec leaves them out', () => {
    registry.add(B, { apiKey: undefined, name: '' });
    assert.equal(registry.get(B).name, 'Unknown Browser');
    assert.equal(registry.get(B).apiKey, '');
  });

  it('closes a driven browser’s driver and hands its vendor session back on removal', async () => {
    const driver = driveBrowser(B, () => ({ ok: true }));
    const release = mock.fn(async () => {});
    registry.get(B).release = release;
    registry.remove(B);
    await new Promise((r) => setImmediate(r));
    assert.equal(driver.closed, true);
    assert.equal(release.mock.callCount(), 1);
  });

  it('logs rather than throws when a release fails', async () => {
    driveBrowser(B, () => ({ ok: true }));
    const logged = mock.method(console, 'error', () => {});
    registry.get(B).release = async () => Promise.reject(new Error('vendor down'));
    registry.remove(B);
    await new Promise((r) => setImmediate(r));
    assert.match(String(logged.mock.calls[0].arguments[1]), /vendor down/);
  });
});

describe('registry: activity', () => {
  afterEach(() => disconnectBrowser(B));

  it('counts a started command as pending until it settles', () => {
    connectBrowser(B);
    registry.commandStarted(B);
    assert.equal(registry.get(B).pending, 1);
    settle(true, { summary: 'x', ms: 12.6 });
    const b = registry.get(B);
    assert.equal(b.pending, 0);
    assert.equal(b.commands, 1);
    assert.deepEqual(
      { ...b.activity[0], ts: undefined },
      { ts: undefined, action: 'click', summary: 'x', ok: true, ms: 13 },
    );
  });

  it('never lets pending go below zero', () => {
    connectBrowser(B);
    settle(true);
    assert.equal(registry.get(B).pending, 0);
  });

  it('keeps a failed command’s error, trimmed', () => {
    connectBrowser(B);
    settle(false, { error: 'e'.repeat(ERROR_CHARS + 50) });
    const b = registry.get(B);
    assert.equal(b.errors, 1);
    assert.equal(b.lastError.length, ERROR_CHARS);
    assert.equal(b.activity[0].error, b.lastError);
  });

  it('names a failure with no message "failed"', () => {
    connectBrowser(B);
    settle(false);
    assert.equal(registry.get(B).lastError, 'failed');
  });

  it('keeps only the most recent commands, newest first', () => {
    connectBrowser(B);
    for (let i = 0; i < ACTIVITY_SIZE + 5; i++) registry.recordActivity(B, { action: `a${i}`, ok: true });
    const { activity } = registry.get(B);
    assert.equal(activity.length, ACTIVITY_SIZE);
    assert.equal(activity[0].action, `a${ACTIVITY_SIZE + 4}`);
  });

  it('ignores activity for a browser it does not hold', () => {
    assert.doesNotThrow(() => {
      registry.commandStarted('missing');
      registry.recordActivity('missing', { action: 'x', ok: true });
      registry.updateLastSeen('missing');
      registry.updateUrl('missing', 'u');
      registry.pushFrame('missing', 'f');
      registry.removeViewer('missing', {});
    });
  });
});

describe('registry: health', () => {
  afterEach(() => {
    disconnectBrowser(B);
    mock.timers.reset();
  });

  it('calls a browser in trouble once enough recent commands fail', () => {
    connectBrowser(B);
    for (let i = 0; i < FAILING_COMMANDS; i++) settle(false);
    assert.equal(registry.describe(B).health, 'errors');
  });

  it('goes stale, then dead, as an inbound browser falls silent', () => {
    mock.timers.enable({ apis: ['Date'], now: 1_000_000 });
    connectBrowser(B);
    mock.timers.tick(STALE_AFTER_MS + 1);
    assert.equal(registry.describe(B).health, 'stale');
    mock.timers.tick(DEAD_AFTER_MS - STALE_AFTER_MS);
    assert.equal(registry.describe(B).health, 'dead');
    registry.updateLastSeen(B);
    assert.equal(registry.describe(B).health, 'ok');
  });

  it('judges a driven browser by its driver, not by silence', () => {
    mock.timers.enable({ apis: ['Date'], now: 1_000_000 });
    const driver = driveBrowser(B, () => ({ ok: true }));
    mock.timers.tick(DEAD_AFTER_MS * 2);
    assert.equal(registry.describe(B).health, 'ok');
    driver.alive = false;
    assert.equal(registry.describe(B).health, 'dead');
  });
});

describe('registry: live view', () => {
  afterEach(() => disconnectBrowser(B));

  it('starts the stream for the first viewer and stops it after the last', () => {
    connectBrowser(B);
    const events = [];
    const start = (e) => events.push(['start', e.id]);
    const stop = (e) => events.push(['stop', e.id]);
    registry.on('stream:start', start);
    registry.on('stream:stop', stop);
    const [one, two] = [new FakeResponse(), new FakeResponse()];
    registry.addViewer(B, one);
    registry.addViewer(B, two);
    assert.equal(registry.hasViewers(B), true);
    registry.removeViewer(B, one);
    registry.removeViewer(B, two);
    registry.off('stream:start', start);
    registry.off('stream:stop', stop);
    assert.deepEqual(events, [
      ['start', B],
      ['stop', B],
    ]);
  });

  it('refuses a viewer for a browser it does not hold', () => {
    assert.equal(registry.addViewer('missing', new FakeResponse()), false);
    assert.equal(registry.hasViewers('missing'), false);
  });

  it('keeps the latest frame and sends it to every viewer as an SSE event', () => {
    connectBrowser(B);
    const viewer = new FakeResponse();
    registry.addViewer(B, viewer);
    registry.pushFrame(B, 'data:image/jpeg;base64,AAA');
    assert.equal(registry.get(B).lastFrame, 'data:image/jpeg;base64,AAA');
    assert.deepEqual(viewer.written, ['data: data:image/jpeg;base64,AAA\n\n']);
  });

  it('skips a frame for a viewer too far behind', () => {
    connectBrowser(B);
    const slow = new FakeResponse();
    slow.writableLength = VIEWER_BACKLOG_BYTES + 1;
    registry.addViewer(B, slow);
    registry.pushFrame(B, 'f');
    assert.deepEqual(slow.written, []);
  });

  it('drops a viewer whose stream throws', () => {
    connectBrowser(B);
    const broken = new FakeResponse();
    broken.write = () => {
      throw new Error('EPIPE');
    };
    registry.addViewer(B, broken);
    registry.pushFrame(B, 'f');
    assert.equal(registry.hasViewers(B), false);
  });

  it('ends every viewer’s stream when the browser goes away', () => {
    connectBrowser(B);
    const viewer = new FakeResponse();
    registry.addViewer(B, viewer);
    disconnectBrowser(B);
    assert.equal(viewer.ended, '');
  });
});

describe('summarise', () => {
  it('keeps the URL being opened', () => {
    assert.equal(summarise('navigate', { url: 'https://example.com' }), 'https://example.com');
  });

  it('says how much was typed, never what', () => {
    assert.equal(summarise('type', { text: 'hunter2' }), '7 chars');
    assert.equal(summarise('keyboard_type', {}), '0 chars');
  });

  it('describes points, drags, scrolls, keys and tabs', () => {
    assert.equal(summarise('click_coordinates', { x: 10.4, y: 20.6 }), '10,21');
    assert.equal(summarise('hover', { selector: '#a' }), '#a');
    assert.equal(summarise('drag', { from_x: 1, from_y: 2, to_x: 3, to_y: 4 }), '1,2 → 3,4');
    assert.equal(summarise('scroll', {}), 'down');
    assert.equal(summarise('scroll', { direction: 'up', amount: 300 }), 'up 300');
    assert.equal(summarise('press_key', { key: 'Enter' }), 'Enter');
    assert.equal(summarise('close_tab', { tab_id: 4 }), '4');
    assert.equal(summarise('click', { element_id: 7 }), '7');
  });

  it('says nothing for other actions or unusable params', () => {
    assert.equal(summarise('evaluate', { expression: 'secret()' }), '');
    assert.equal(summarise('navigate', null), '');
    assert.equal(summarise('toString', {}), '');
  });
});
