/**
 * Unit tests for the one place that decides what an element can be found by.
 *
 * The cases here are the failures they came from: a Send button whose only id was
 * Ember's, a nav link renamed by a notification count, and a composer where the
 * emoji picker and the expand-to-full-screen button sit either side of Send.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const {
  HANDLES,
  handlesOf,
  contradicts,
  missingIdentity,
  stableId,
  withoutLiveCount,
} = require('../../../../scripts/workflow/handles.cjs');

describe('the order of trust', () => {
  it('puts what the page author wrote before what a renderer invented', () => {
    assert.deepEqual(
      HANDLES.map((h) => h.kind),
      ['testId', 'href', 'domId', 'ariaLabel', 'text', 'name', 'placeholder', 'path'],
    );
  });

  it('offers a link its own target before any id', () => {
    assert.deepEqual(
      handlesOf({ tag: 'a', domId: 'mwCg', rawHref: '/wiki/X' }).map((h) => h.kind),
      ['href'],
    );
  });

  it('offers nothing of an id a framework made up', () => {
    assert.deepEqual(
      handlesOf({ tag: 'button', domId: 'ember80', text: 'Send' }).map((h) => h.kind),
      ['text'],
    );
  });

  it('reads a label without the count a page keeps changing', () => {
    const [handle] = handlesOf({ tag: 'a', ariaLabel: 'Messaging, 0 new notifications' });
    assert.equal(handle.of({ ariaLabel: 'Messaging, 3 new notifications' }), 'Messaging');
  });
});

describe('stableId', () => {
  it('keeps an id a person wrote and refuses one a render made up', () => {
    for (const id of ['search-conversations', 'authWizardNextButton']) assert.equal(stableId(id), true, id);
    for (const id of ['ember80', 'mwCg', ':r3:', 'ext-gen1023', 'user_1234567890'])
      assert.equal(stableId(id), false, id);
  });
});

describe('withoutLiveCount', () => {
  it('drops a count clause and leaves real numbers alone', () => {
    assert.equal(withoutLiveCount('Messaging, 0 new notifications'), 'Messaging');
    assert.equal(withoutLiveCount('Inbox (12)'), 'Inbox');
    assert.equal(withoutLiveCount('Page 2'), 'Page 2');
    assert.equal(withoutLiveCount('Top 10 lists'), 'Top 10 lists');
  });
});

describe('contradicts', () => {
  const send = { tag: 'button', ariaLabel: 'Send', path: 'div > button:nth-of-type(3)' };

  it('refuses the sibling that took the same slot', () => {
    const emoji = { tag: 'button', ariaLabel: 'Open Emoji Keyboard', path: 'div > button:nth-of-type(3)' };
    assert.equal(contradicts(send, emoji), true);
  });

  it('accepts the same control after it moved, because a place is not an identity', () => {
    assert.equal(contradicts(send, { ...send, path: 'div > button:nth-of-type(4)' }), false);
  });

  it('refuses two different test ids however much else agrees', () => {
    assert.equal(contradicts({ testId: 'send', tag: 'button' }, { testId: 'emoji', tag: 'button' }), true);
  });

  it('says nothing about a handle only one side has', () => {
    assert.equal(contradicts({ tag: 'button', ariaLabel: 'Send' }, { tag: 'button', testId: 'send' }), false);
  });
});

describe('missingIdentity', () => {
  it('refuses a position whose occupant has lost the name that was recorded', () => {
    const send = { tag: 'button', ariaLabel: 'Send', path: 'div > button:nth-of-type(3)' };
    assert.equal(missingIdentity(send, { tag: 'button', path: 'div > button:nth-of-type(3)' }), true);
  });

  it('allows a position when position was all the recording had', () => {
    assert.equal(missingIdentity({ tag: 'div', path: 'p > div' }, { tag: 'div', path: 'p > div' }), false);
  });
});
