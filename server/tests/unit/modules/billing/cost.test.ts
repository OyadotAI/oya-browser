/**
 * Unit tests for what a hosted model call cost.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { llmCost } from '../../../../src/modules/billing/cost.ts';

describe('llmCost', () => {
  it('prices a known model’s input and output tokens', () => {
    assert.equal(llmCost('gpt-4o-mini', { prompt_tokens: 1000, completion_tokens: 100 }), 210);
  });

  it('reads an OpenRouter-style name as the bare model', () => {
    assert.equal(llmCost('openai/gpt-4o-mini', { prompt_tokens: 1000 }), 150);
  });

  it('prices a model it does not know at the dearest, never free', () => {
    assert.equal(llmCost('gpt-9', { prompt_tokens: 1000, completion_tokens: 100 }), 3500);
  });

  it('costs nothing when no tokens were reported', () => {
    assert.equal(llmCost('gpt-4o-mini', undefined), 0);
  });
});
