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
  volatileTarget,
} = require('../../../../scripts/workflow/handles.cjs');

describe('the order of trust', () => {
  it('puts what the page author wrote before what a renderer invented', () => {
    assert.deepEqual(
      HANDLES.map((h) => h.kind),
      ['testId', 'href', 'domId', 'ariaLabel', 'scoped', 'text', 'name', 'placeholder', 'path'],
    );
  });

  it('prefers a name scoped to its own row over the bare name', () => {
    assert.deepEqual(
      handlesOf({ tag: 'button', text: 'View', scoped: '[data-row="7"] button:text-is("View")' }).map((h) => h.kind),
      ['scoped', 'text'],
    );
  });

  it('offers nothing of a name the page repeats, since it never said which one', () => {
    assert.deepEqual(
      handlesOf({ tag: 'button', text: 'Delete', repeats: 'true', path: 'li:nth-of-type(2) > button' }).map(
        (h) => h.kind,
      ),
      ['path'],
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

describe('a link target across two visits', () => {
  const { stableTarget } = require('../../../../scripts/workflow/handles.cjs');

  it('ignores the request and session ids a site regenerates per visit', () => {
    // Amazon's pagination: qid is an epoch, xpid a session. Same link, new noise.
    const recorded = '/s?k=mechanical+keyboard&page=2&xpid=6lALgpQXYt46F&qid=1789935022&ref=sr_pg_2';
    const live = '/s?k=mechanical+keyboard&page=2&xpid=ZZZZZZ&qid=1789999999&ref=sr_pg_2';
    assert.equal(stableTarget(recorded), stableTarget(live));
  });

  it('still tells page 2 from page 3, which is what the query is for', () => {
    const two = '/s?k=kb&page=2&qid=1&ref=sr_pg_2';
    const three = '/s?k=kb&page=3&qid=2&ref=sr_pg_3';
    assert.notEqual(stableTarget(two), stableTarget(three));
  });

  it('leaves a plain link alone', () => {
    assert.equal(
      stableTarget('https://en.wikipedia.org/wiki/Managed_care'),
      'https://en.wikipedia.org/wiki/Managed_care',
    );
    assert.equal(stableTarget('/r/programming/comments/abc/title/'), '/r/programming/comments/abc/title/');
  });

  it('says which targets cannot be matched literally, and which can', () => {
    assert.equal(volatileTarget('/s?k=kb&page=2&qid=1789940643&xpid=njio9'), true);
    assert.equal(volatileTarget('/s?k=kb&page=2'), false);
    // No query to be noisy, and nothing at all: both are matchable as written.
    assert.equal(volatileTarget('https://news.ycombinator.com'), false);
    assert.equal(volatileTarget(undefined), false);
  });

  it('drops the tracking tags that follow a shared link around', () => {
    assert.equal(stableTarget('/p?id=7&utm_source=x&gclid=y&trk=z'), stableTarget('/p?id=7'));
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
