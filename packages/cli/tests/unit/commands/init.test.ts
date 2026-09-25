/**
 * Unit tests for `oya init`'s model step (src/commands/init.ts): what an LLM
 * choice saves, so the model never outlives the provider it was chosen for.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { llmUpdates } from '../../../src/commands/init.ts';

/** A key on OpenAI with its own credential. */
const ON_OPENAI = { providers: [], has_openai_key: true, chat_model: 'gpt-4.1', llm_provider: 'openai' };

describe('oya init: the model step', () => {
  it('sends the provider’s default as null when no model is typed, never a leftover from the last provider', () => {
    assert.deepEqual(llmUpdates(ON_OPENAI, 'openrouter', 'sk-or-1', ''), {
      llm_provider: 'openrouter',
      chat_model: null,
      openai_api_key: 'sk-or-1',
      openai_base_url: null,
    });
  });

  it('keeps the key and endpoint when the provider stays the same', () => {
    assert.deepEqual(llmUpdates(ON_OPENAI, 'openai', '', 'gpt-6-sol'), {
      llm_provider: 'openai',
      chat_model: 'gpt-6-sol',
    });
  });
});
