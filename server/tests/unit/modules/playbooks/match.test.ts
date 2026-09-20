/**
 * Unit tests for finding a recorded element on the live page by its stable
 * handles, most stable first, preferring visible elements.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { matchElement } from '../../../../src/modules/playbooks/match.ts';

/** A live element. */
const live = (id, fields = {}) => ({ id, visible: true, ...fields });

describe('matchElement', () => {
  it('matches by test id before anything else', () => {
    const els = [live(1, { domId: 'd' }), live(2, { testId: 't' })];
    assert.equal(matchElement({ testId: 't', domId: 'd' }, els).id, 2);
  });

  it('matches by DOM id when there is no test id', () => {
    assert.equal(matchElement({ domId: 'd' }, [live(1), live(2, { domId: 'd' })]).id, 2);
  });

  it('matches an aria label, a name and a placeholder only on the same tag', () => {
    const els = [live(1, { ariaLabel: 'Q', tag: 'div' }), live(2, { ariaLabel: 'Q', tag: 'input' })];
    assert.equal(matchElement({ ariaLabel: 'Q', tag: 'input' }, els).id, 2);
    assert.equal(matchElement({ name: 'n', tag: 'select' }, [live(3, { name: 'n', tag: 'input' })]), null);
    assert.equal(matchElement({ placeholder: 'p', tag: 'input' }, [live(4, { placeholder: 'p', tag: 'input' })]).id, 4);
  });

  it('matches visible text only among elements of the same type', () => {
    const els = [live(1, { text: 'Go', type: 'link' }), live(2, { text: 'Go', type: 'button' })];
    assert.equal(matchElement({ text: 'Go', type: 'button' }, els).id, 2);
  });

  it('matches a link by its href last', () => {
    assert.equal(matchElement({ href: '/x' }, [live(1, { href: '/x' })]).id, 1);
  });

  it('prefers a visible element over an off-screen one with the same handle', () => {
    const els = [{ id: 1, visible: false, testId: 't' }, live(2, { testId: 't' })];
    assert.equal(matchElement({ testId: 't' }, els).id, 2);
  });

  it('finds a data-driven click by text alone, ignoring case and spaces', () => {
    const els = [live(1, { testId: 't', text: 'Basic' }), live(2, { text: '  PRO ' })];
    assert.equal(matchElement({ testId: 't' }, els, 'pro').id, 2);
    assert.equal(matchElement({ testId: 't' }, els, 'Enterprise'), null);
  });

  it('answers null when nothing matches, or nothing was recorded', () => {
    assert.equal(matchElement({ testId: 'gone' }, [live(1)]), null);
    assert.equal(matchElement(undefined, [live(1)], undefined), null);
  });
});
describe('matchElement refuses the wrong neighbour', () => {
  /** A message composer: emoji, expand and send are siblings of the same shape. */
  const composer = [
    {
      id: 1,
      tag: 'button',
      type: 'button',
      ariaLabel: 'Open Emoji Keyboard',
      path: 'div > button:nth-of-type(2)',
      visible: true,
    },
    {
      id: 2,
      tag: 'button',
      type: 'button',
      ariaLabel: 'Expand to full screen',
      path: 'div > button:nth-of-type(3)',
      visible: true,
    },
    { id: 3, tag: 'button', type: 'button', ariaLabel: 'Send', path: 'div > button:nth-of-type(4)', visible: true },
  ];

  it('finds Send by its name even when the buttons have been reordered', () => {
    const recorded = { tag: 'button', type: 'button', ariaLabel: 'Send', path: 'div > button:nth-of-type(3)' };
    assert.equal(matchElement(recorded, composer)?.id, 3);
  });

  it('does not take the button that now sits where Send used to', () => {
    const recorded = { tag: 'button', type: 'button', ariaLabel: 'Send', path: 'div > button:nth-of-type(2)' };
    const found = matchElement(recorded, composer);
    assert.notEqual(found?.ariaLabel, 'Open Emoji Keyboard');
    assert.equal(found?.ariaLabel, 'Send');
  });

  it('will not match an id a framework invented, even when it is still on the page', () => {
    const pool = [
      { id: 7, tag: 'button', type: 'button', domId: 'ember80', ariaLabel: 'Report this post', visible: true },
    ];
    assert.equal(matchElement({ tag: 'button', type: 'button', domId: 'ember80', ariaLabel: 'Send' }, pool), null);
  });

  it('matches a nav link whose count has changed since it was recorded', () => {
    const pool = [{ id: 4, tag: 'a', type: 'link', ariaLabel: 'Messaging, 3 new notifications', visible: true }];
    const recorded = { tag: 'a', type: 'link', ariaLabel: 'Messaging, 0 new notifications' };
    assert.equal(matchElement(recorded, pool)?.id, 4);
  });
});
describe('matchElement across two visits of the same link', () => {
  it("finds Amazon's page-2 link although its request and session ids changed", () => {
    const pool = [
      { id: 1, tag: 'a', type: 'link', rawHref: '/s?k=kb&page=2&xpid=NEW&qid=1789999999&ref=sr_pg_2', visible: true },
      { id: 2, tag: 'a', type: 'link', rawHref: '/s?k=kb&page=3&xpid=NEW&qid=1789999999&ref=sr_pg_3', visible: true },
    ];
    const recorded = { tag: 'a', type: 'link', rawHref: '/s?k=kb&page=2&xpid=OLD&qid=1789935022&ref=sr_pg_2' };
    assert.equal(matchElement(recorded, pool)?.id, 1);
  });

  it('does not take page 3 for page 2, because the query is what tells them apart', () => {
    const pool = [{ id: 2, tag: 'a', type: 'link', rawHref: '/s?k=kb&page=3&qid=2&ref=sr_pg_3', visible: true }];
    const recorded = { tag: 'a', type: 'link', rawHref: '/s?k=kb&page=2&qid=1&ref=sr_pg_2' };
    assert.equal(matchElement(recorded, pool), null);
  });
});

