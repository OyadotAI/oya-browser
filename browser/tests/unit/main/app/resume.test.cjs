/**
 * Unit tests for a signed-in launch: the window opens straight on the Oya start
 * page, which loads nothing from the web; a browser without a key or a persona
 * waits for the server instead.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { resumeSignedIn } = require('../../../../main/app/resume.cjs');
const { HOME_URL } = require('../../../../main/tabs/constants.cjs');
const { mainCtx } = require('../../support/main-ctx.cjs');

/** A context that records where browsing was entered, signed in unless told otherwise. */
function launch({ apiKey = 'k', persona = { id: 'p1' } } = {}) {
  const ctx = mainCtx();
  ctx.config.values = { apiKey };
  ctx.persona.active = persona;
  ctx.tabs = { enterBrowsingMode: (url) => (ctx.entered = url) };
  return ctx;
}

describe('resumeSignedIn', () => {
  it('opens browsing on the start page when this desktop has signed in before', () => {
    const ctx = launch();
    resumeSignedIn(ctx);
    assert.equal(ctx.entered, HOME_URL);
  });

  it('waits for the server without a saved key or persona', () => {
    for (const ctx of [launch({ apiKey: '' }), launch({ persona: null })]) {
      resumeSignedIn(ctx);
      assert.equal(ctx.entered, undefined);
    }
  });
});
