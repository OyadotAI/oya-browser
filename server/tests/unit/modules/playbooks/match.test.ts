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
