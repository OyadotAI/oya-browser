/**
 * Unit tests for scripts/recording/frames.cjs: the selector for a frame's owner
 * element, which a replay finds the frame by.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { frameSelector } = require('../../../../scripts/recording/frames.cjs');

/** A transport whose frame owner has these attributes. */
const ownerWith = (attributes) => async (method) =>
  method === 'DOM.getFrameOwner' ? { backendNodeId: 1 } : { node: { localName: 'iframe', attributes } };

describe('frameSelector', () => {
  it('finds a frame by its path when its address is all it has, since embeds make up hosts and queries per load', async () => {
    const src = 'https://8169bdc7.mdnplay.dev/runner.html?uuid=8169bdc7&state=abc';
    assert.equal(await frameSelector(ownerWith(['src', src]), 'f'), 'iframe[src*="/runner.html"]');
  });

  it('prefers a title to the address', async () => {
    const selector = await frameSelector(ownerWith(['src', 'https://x.test/a', 'title', 'Payment']), 'f');
    assert.equal(selector, 'iframe[title="Payment"]');
  });
});
