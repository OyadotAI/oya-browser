/**
 * Every way a page can change under a recording, as a table.
 *
 * Each case is a recorded handle, the live page it meets, and whether the two are
 * the same element. These are the failures the live site matrix found, plus the
 * ones it would have found eventually: a matcher regression shows up here in a
 * second instead of a quarter of an hour.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { matchElement } from '../../../../src/modules/playbooks/match.ts';

/** A live element, with the fields the analyzer reports. */
const el = (over: any = {}) => ({ id: 1, tag: 'button', type: 'button', visible: true, ...over });

type Case = {
  /** What it is, in the words of the failure it came from. */
  name: string;
  /** The handle the recording kept. */
  recorded: any;
  /** The page as it is now. */
  live: any[];
  /** The id that must be matched, or null when nothing may be. */
  expect: number | null;
};

const CASES: Case[] = [
  // ── The same element, changed in ways that must not matter ──
  {
    name: 'a nav link whose notification count moved on',
    recorded: { tag: 'a', type: 'link', ariaLabel: 'Messaging, 0 new notifications' },
    live: [el({ id: 10, tag: 'a', type: 'link', ariaLabel: 'Messaging, 7 new notifications' })],
    expect: 10,
  },
  {
    name: 'a link whose request and session ids were reissued',
    recorded: { tag: 'a', type: 'link', rawHref: '/s?k=kb&page=2&qid=111&xpid=AAA' },
    live: [el({ id: 11, tag: 'a', type: 'link', rawHref: '/s?k=kb&page=2&qid=999&xpid=ZZZ' })],
    expect: 11,
  },
  {
    name: 'a link that picked up tracking tags',
    recorded: { tag: 'a', type: 'link', rawHref: '/post?id=7' },
    live: [el({ id: 12, tag: 'a', type: 'link', rawHref: '/post?id=7&utm_source=news&gclid=abc' })],
    expect: 12,
  },
  {
    name: 'a button that moved to another place on the page',
    recorded: { tag: 'button', type: 'button', ariaLabel: 'Send', path: 'div > button:nth-of-type(3)' },
    live: [el({ id: 13, ariaLabel: 'Send', path: 'section > div > button:nth-of-type(5)' })],
    expect: 13,
  },
  {
    name: 'a field whose label gained a required marker',
    recorded: { tag: 'input', type: 'input', name: 'memberId', text: 'Member ID' },
    live: [el({ id: 14, tag: 'input', type: 'input', name: 'memberId', text: 'Member ID *' })],
    expect: 14,
  },
  {
    name: 'a test id still on the page though everything else changed',
    recorded: { tag: 'button', type: 'button', testId: 'submit-claim', text: 'Submit' },
    live: [el({ id: 15, testId: 'submit-claim', text: 'Submit and continue' })],
    expect: 15,
  },

  // ── A different element, however much it looks alike ──
  {
    name: 'the sibling that took the slot the send button had',
    recorded: { tag: 'button', type: 'button', ariaLabel: 'Send', path: 'div > button:nth-of-type(3)' },
    live: [el({ id: 20, ariaLabel: 'Open Emoji Keyboard', path: 'div > button:nth-of-type(3)' })],
    expect: null,
  },
  {
    name: 'an unlabelled control standing where a named one was recorded',
    recorded: { tag: 'button', type: 'button', ariaLabel: 'Send', path: 'div > button:nth-of-type(3)' },
    live: [el({ id: 21, path: 'div > button:nth-of-type(3)' })],
    expect: null,
  },
  {
    name: "a framework's id now belonging to another control",
    recorded: { tag: 'button', type: 'button', domId: 'ember80', ariaLabel: 'Send' },
    live: [el({ id: 22, domId: 'ember80', ariaLabel: 'Report this post' })],
    expect: null,
  },
  {
    name: "Wikipedia's per-render id, reused by a different link",
    recorded: { tag: 'a', type: 'link', domId: 'mwCg', text: 'Managed care' },
    live: [el({ id: 23, tag: 'a', type: 'link', domId: 'mwCg', text: 'Utilization review' })],
    expect: null,
  },
  {
    name: 'page 3 offered where page 2 was recorded',
    recorded: { tag: 'a', type: 'link', rawHref: '/s?k=kb&page=2&qid=1' },
    live: [el({ id: 24, tag: 'a', type: 'link', rawHref: '/s?k=kb&page=3&qid=2' })],
    expect: null,
  },
  {
    name: 'the same test id reused by a destructive button',
    recorded: { tag: 'button', type: 'button', testId: 'primary', ariaLabel: 'Submit claim' },
    live: [el({ id: 25, testId: 'primary', ariaLabel: 'Delete account' })],
    expect: null,
  },

  // ── Gone, ambiguous, or out of reach ──
  {
    name: 'an element that has left the page',
    recorded: { tag: 'button', type: 'button', testId: 'gone', ariaLabel: 'Send' },
    live: [el({ id: 30, testId: 'other', ariaLabel: 'Cancel' })],
    expect: null,
  },
  {
    name: 'the visible one of two that share a name',
    recorded: { tag: 'button', type: 'button', text: 'Next' },
    live: [el({ id: 31, text: 'Next', visible: false }), el({ id: 32, text: 'Next', visible: true })],
    expect: 32,
  },
  {
    name: 'a name that repeats, told apart by its own test id',
    recorded: { tag: 'button', type: 'button', text: 'View', testId: 'row-7-view' },
    live: [el({ id: 33, text: 'View', testId: 'row-3-view' }), el({ id: 34, text: 'View', testId: 'row-7-view' })],
    expect: 34,
  },
  {
    name: 'a handle of a kind the page no longer offers at all',
    recorded: { tag: 'input', type: 'input', placeholder: 'Search' },
    live: [el({ id: 35, tag: 'input', type: 'input' })],
    expect: null,
  },
  {
    name: 'text that reads the same in a different kind of element',
    recorded: { tag: 'button', type: 'button', text: 'Apply' },
    live: [el({ id: 36, tag: 'a', type: 'link', text: 'Apply' })],
    expect: null,
  },
  {
    name: 'nothing recorded at all',
    recorded: {},
    live: [el({ id: 37, text: 'Anything' })],
    expect: null,
  },
  {
    name: 'an empty page',
    recorded: { tag: 'button', type: 'button', text: 'Send' },
    live: [],
    expect: null,
  },

  // ── Several controls answer to the same name ──
  {
    name: 'the second Delete of three, told apart by where it sits',
    recorded: { tag: 'button', type: 'button', text: 'Delete', path: 'li:nth-of-type(2) > button' },
    live: [
      el({ id: 60, text: 'Delete', path: 'li:nth-of-type(1) > button' }),
      el({ id: 61, text: 'Delete', path: 'li:nth-of-type(2) > button' }),
      el({ id: 62, text: 'Delete', path: 'li:nth-of-type(3) > button' }),
    ],
    expect: 61,
  },
  {
    name: 'a repeated name the analyzer could scope to its own row',
    recorded: { tag: 'button', type: 'button', text: 'View', scoped: '[data-row="7"] button:text-is("View")' },
    live: [
      el({ id: 63, text: 'View', scoped: '[data-row="6"] button:text-is("View")' }),
      el({ id: 64, text: 'View', scoped: '[data-row="7"] button:text-is("View")' }),
    ],
    expect: 64,
  },
  {
    name: 'a name the analyzer marked as repeating, with nothing else to go on',
    recorded: { tag: 'button', type: 'button', text: 'Delete', repeats: 'true' },
    live: [el({ id: 65, text: 'Delete' }), el({ id: 66, text: 'Delete' })],
    // Withdrawn as a handle: it never said which one, and guessing the first is how
    // the wrong row got deleted. A step with nothing else recorded fails instead.
    expect: null,
  },
  {
    name: 'a slot whose occupant lost the repeated name that was recorded',
    recorded: { tag: 'button', type: 'button', text: 'Delete', repeats: 'true', path: 'li:nth-of-type(2) > button' },
    // The name cannot say which Delete this was, so it is not used to find it, but a
    // button in that slot saying nothing at all is not the button that was recorded.
    live: [el({ id: 71, text: '', path: 'li:nth-of-type(2) > button' })],
    expect: null,
  },
  {
    name: 'an ambiguous name still answers when no handle can do better',
    recorded: { tag: 'button', type: 'button', text: 'Send' },
    live: [el({ id: 67, text: 'Send' }), el({ id: 68, text: 'Send' })],
    expect: 67,
  },
  {
    name: 'a unique name lower down beats an ambiguous one above it',
    recorded: { tag: 'input', type: 'input', ariaLabel: 'Search', name: 'q2' },
    live: [
      el({ id: 69, tag: 'input', type: 'input', ariaLabel: 'Search', name: 'q1' }),
      el({ id: 70, tag: 'input', type: 'input', ariaLabel: 'Search', name: 'q2' }),
    ],
    expect: 70,
  },
];

describe('matchElement, case by case', () => {
  for (const c of CASES) {
    it(c.name, () => {
      const found = matchElement(c.recorded, c.live as any);
      assert.equal(found?.id ?? null, c.expect);
    });
  }
});

describe('a data-driven click aims by value', () => {
  it('takes the element now showing the filled-in value', () => {
    const live = [el({ id: 40, text: 'Basic' }), el({ id: 41, text: 'Pro' })];
    assert.equal(matchElement({ tag: 'button', type: 'button', text: '{{plan}}' }, live as any, 'Pro')?.id, 41);
  });

  it('matches the value however it is cased or spaced', () => {
    const live = [el({ id: 42, text: '  PRO  ' })];
    assert.equal(matchElement({ tag: 'button', type: 'button' }, live as any, 'pro')?.id, 42);
  });

  it('finds nothing when no element carries the value', () => {
    assert.equal(matchElement({ tag: 'button', type: 'button' }, [el({ id: 43, text: 'Basic' })] as any, 'Gold'), null);
  });
});
