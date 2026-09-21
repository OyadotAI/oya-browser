/**
 * Unit tests for trimming a long run's conversation: the oldest tool results go
 * first, with the assistant turn that asked for them, and the task always stays.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { trimContext, condenseOldPages } from '../../../../src/modules/agent/context.ts';
import { MAX_CONTEXT_CHARS } from '../../../../src/modules/agent/constants.ts';

const BIG = 'x'.repeat(MAX_CONTEXT_CHARS / 2);
/** A tool result too short to be a page read: an answer from click or type. */
const SHORT = 'y'.repeat(1_500);
/** A page read, as analyze_page returns one. */
const PAGE = `---\nurl: https://a.test/page\nelements: 42 total, 30 visible\n---\n\n${'page text '.repeat(3000)}`;

/** An assistant turn calling `n` tools, and their results. */
const toolTurn = (tag, n = 1, content = BIG) => [
  { role: 'assistant', content: null, tool_calls: Array.from({ length: n }, (_, i) => ({ id: `${tag}${i}` })) },
  ...Array.from({ length: n }, (_, i) => ({ role: 'tool', tool_call_id: `${tag}${i}`, content })),
];

const HEAD = [
  { role: 'system', content: 'sys' },
  { role: 'user', content: 'task' },
];

describe('condenseOldPages', () => {
  it('keeps the last pages whole and leaves a note where the older ones were', () => {
    const messages = [...HEAD, ...toolTurn('a', 1, PAGE), ...toolTurn('b', 1, PAGE), ...toolTurn('c', 1, PAGE)];
    condenseOldPages(messages);
    const [oldest, ...kept] = messages.filter((m) => m.role === 'tool');
    assert.match(oldest.content, /earlier page read of https:\/\/a\.test\/page, 42 elements dropped/);
    assert.match(oldest.content, /analyze_page again/);
    assert.deepEqual(
      kept.map((m) => m.content),
      [PAGE, PAGE],
      'the last two pages stay whole',
    );
  });

  it('keeps only what it is asked to keep', () => {
    const messages = [...HEAD, ...toolTurn('a', 1, PAGE), ...toolTurn('b', 1, PAGE)];
    condenseOldPages(messages, 1);
    const [older, newest] = messages.filter((m) => m.role === 'tool');
    assert.match(older.content, /earlier page read/);
    assert.equal(newest.content, PAGE);
  });

  it('leaves short tool results alone: they are answers, not pages', () => {
    const messages = [...HEAD, ...toolTurn('a', 1, 'Clicked element 3'), ...toolTurn('b', 1, PAGE)];
    condenseOldPages(messages);
    assert.equal(messages.filter((m) => m.role === 'tool')[0].content, 'Clicked element 3');
  });
});

describe('trimContext', () => {
  it('leaves a conversation under the limit alone', () => {
    const messages = [...HEAD, ...toolTurn('a', 1, 'small')];
    trimContext(messages);
    assert.equal(messages.length, 4);
  });

  it('keeps every turn when noting the older pages is enough to fit', () => {
    const messages = [...HEAD, ...toolTurn('a', 2, PAGE), ...toolTurn('b', 1, PAGE)];
    trimContext(messages);
    assert.deepEqual(
      messages.map((m) => m.tool_call_id || m.role),
      ['system', 'user', 'assistant', 'a0', 'a1', 'assistant', 'b0'],
    );
  });

  it('drops the oldest turns when short results alone fill the window', () => {
    const many = Array.from({ length: 400 }, (_, i) => toolTurn(`t${i}`, 1, SHORT)).flat();
    const messages = [...HEAD, ...many];
    trimContext(messages);
    assert.ok(messages.length < many.length, 'older turns went');
    assert.deepEqual(
      messages.slice(0, 2).map((m) => m.role),
      ['system', 'user'],
      'the task stays',
    );
    assert.ok(
      messages.reduce((n, m) => n + (m.content?.length || 0), 0) <= MAX_CONTEXT_CHARS,
      'what is left fits the window',
    );
  });

  it('drops a lone tool result that has no assistant turn before it', () => {
    const many = Array.from({ length: 400 }, () => ({ role: 'tool', content: SHORT }));
    const messages = [...HEAD, ...many, { role: 'user', content: 'x' }];
    trimContext(messages);
    assert.ok(messages.filter((m) => m.role === 'tool').length < 400, 'lone results go one by one');
  });

  it('stops when there are no tool results left to drop', () => {
    const messages = [...HEAD, { role: 'user', content: BIG + BIG + BIG }];
    trimContext(messages);
    assert.equal(messages.length, 3);
  });

  it('never trims below the system prompt and the task', () => {
    const messages = [
      { role: 'system', content: 'sys' },
      { role: 'tool', content: BIG + BIG + BIG },
    ];
    trimContext(messages);
    assert.equal(messages.length, 2);
  });
});
