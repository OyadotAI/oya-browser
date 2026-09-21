/**
 * Unit tests for the MCP reply shapes and attempt(), which turns thrown
 * errors into values.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { attempt, fail, text } from '../../../src/mcp/replies.ts';

describe('replies', () => {
  it('wraps text as one text content item', () => {
    assert.deepEqual(text('hi'), { content: [{ type: 'text', text: 'hi' }] });
  });

  it('marks a failure as an error the model sees', () => {
    assert.deepEqual(fail('nope'), { content: [{ type: 'text', text: 'Error: nope' }], isError: true });
  });

  it('returns the work’s value, or what it threw', async () => {
    assert.deepEqual(await attempt(async () => 5), { value: 5 });
    const err = new Error('x');
    assert.deepEqual(
      await attempt(async () => {
        throw err;
      }),
      { error: err },
    );
  });
});
