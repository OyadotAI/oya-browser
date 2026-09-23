/**
 * Unit tests for a signed-in launch: no site loads before the server's cookies
 * are in the jar, the home page loads once they are (or once the server has
 * not answered in time), and a tab the person already used is left alone.
 */
const { describe, it, beforeEach, afterEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const { resumeSignedIn, openResumedHome } = require('../../../../main/app/resume.cjs');
const { RESUME_OFFLINE_MS } = require('../../../../main/app/constants.cjs');
const { mainCtx } = require('../../support/main-ctx.cjs');

/** Lets pending promise callbacks run. */
const settle = () => new Promise((resolve) => setImmediate(resolve));

/** A signed-in context whose one tab records what it loads. */
function signedIn() {
  const ctx = mainCtx();
  ctx.config.values = { apiKey: 'k' };
  ctx.persona.active = { id: 'p1' };
  const contents = { url: '', loaded: [], getURL: () => contents.url };
  contents.loadURL = async (url) => contents.loaded.push(url);
  // A protected tab: loadInTab loads at once.
  const tab = { id: 1, view: { webContents: contents }, setup: Promise.resolve(true), protection: 'ready' };
  ctx.tabs = {
    activeTabId: 1,
    find: (id) => (id === 1 ? tab : undefined),
    enterBrowsingMode: (url) => (ctx.entered = url),
  };
  return { ctx, contents };
}

describe('resumeSignedIn', () => {
  beforeEach(() => mock.timers.enable({ apis: ['setTimeout'] }));
  afterEach(() => mock.timers.reset());

  it('opens browsing on a blank tab, loading no site before the server answers', () => {
    const { ctx, contents } = signedIn();
    resumeSignedIn(ctx);
    assert.equal(ctx.entered, 'about:blank');
    assert.deepEqual(contents.loaded, []);
  });

  it('loads the home page once the server has answered, and only once', async () => {
    const { ctx, contents } = signedIn();
    resumeSignedIn(ctx);
    openResumedHome(ctx);
    openResumedHome(ctx);
    mock.timers.tick(RESUME_OFFLINE_MS);
    await settle();
    assert.deepEqual(contents.loaded, ['https://google.com']);
  });

  it('loads the home page anyway when the server has not answered in time', async () => {
    const { ctx, contents } = signedIn();
    resumeSignedIn(ctx);
    mock.timers.tick(RESUME_OFFLINE_MS);
    await settle();
    assert.deepEqual(contents.loaded, ['https://google.com']);
  });

  it('leaves a tab the person already went somewhere in alone', () => {
    const { ctx, contents } = signedIn();
    resumeSignedIn(ctx);
    contents.url = 'https://example.com/';
    openResumedHome(ctx);
    assert.deepEqual(contents.loaded, []);
  });

  it('does nothing on a reconnect that was not a resumed launch', () => {
    const { ctx, contents } = signedIn();
    openResumedHome(ctx);
    assert.deepEqual(contents.loaded, []);
  });
});
