/**
 * Unit tests for the settings dialog's pure rules: provider inference, draft
 * edits, provider switches and tab navigation.
 */
import { describe, it, expect } from 'vitest';
import type { KeyConfig } from '@/components/dashboard/config';
import {
  presetModels,
  savedProvider,
  switchProvider,
  withField,
  withModelPair,
} from '@/components/dashboard/settings/model';
import { LLM_CATALOG } from '../../../support/llm-catalog';
import { nextTab } from '@/components/dashboard/settings/tabs';

/** A config that carries the server's catalog. */
const WITH_CATALOG = { llm_catalog: LLM_CATALOG } as KeyConfig;

/** A config with only what provider inference reads. */
const config = (llm_provider: string, baseUrl: string) => ({ llm_provider, effective: { baseUrl } }) as KeyConfig;

describe('savedProvider', () => {
  it('uses the saved provider when there is one', () => expect(savedProvider(config('gemini', ''))).toBe('gemini'));
  it('infers the provider from the base URL', () => {
    expect(savedProvider(config('', 'https://api.anthropic.com/v1'))).toBe('anthropic');
    expect(savedProvider(config('', 'https://x-aiplatform.googleapis.com'))).toBe('vertex');
    expect(savedProvider(config('', 'https://generativelanguage.googleapis.com'))).toBe('gemini');
    expect(savedProvider(config('', 'https://openrouter.ai/api/v1'))).toBe('openrouter');
  });
  it('falls back to OpenAI-compatible', () => {
    expect(savedProvider(config('', 'https://gateway.example'))).toBe('openai');
    expect(savedProvider(null)).toBe('openai');
  });
});

describe('draft rules', () => {
  it('keeps a changed field and drops one set back to its saved value', () => {
    expect(withField({}, 'a', 'x', 'saved')).toEqual({ a: 'x' });
    expect(withField({ a: 'x', b: 'y' }, 'a', 'saved', 'saved')).toEqual({ b: 'y' });
  });
  it('switching provider picks its default model and drops the key and base URL', () => {
    const next = switchProvider(
      WITH_CATALOG,
      { openai_api_key: 'sk', openai_base_url: 'https://g', captcha_solver: 'c' },
      'anthropic',
    );
    expect(next).toEqual({
      captcha_solver: 'c',
      llm_provider: 'anthropic',
      chat_model: 'claude-opus-5',
      openai_base_url: '',
    });
  });
  it('offers the models the server lists for a provider, and none for an unknown one', () => {
    expect(presetModels(WITH_CATALOG, 'openrouter').map((m) => m.id)).toEqual([
      'anthropic/claude-sonnet-5',
      'x-ai/grok-4.7',
    ]);
    expect(presetModels(WITH_CATALOG, 'toString')).toEqual([]);
    expect(presetModels(null, 'openai')).toEqual([]);
  });
  it('always saves the provider and the model together, so a stale dialog cannot split them', () => {
    expect(withModelPair({ chat_model: 'gpt-4.1' }, { provider: 'openai', model: 'gpt-4.1' })).toEqual({
      llm_provider: 'openai',
      chat_model: 'gpt-4.1',
    });
    expect(withModelPair({ captcha_solver: 'c' }, { provider: 'openai', model: 'gpt-4.1' })).toEqual({
      captcha_solver: 'c',
    });
  });
});

describe('nextTab', () => {
  it('wraps arrows around both ends', () => {
    expect(nextTab('ArrowRight', 4, 5)).toBe(0);
    expect(nextTab('ArrowUp', 0, 5)).toBe(4);
    expect(nextTab('ArrowDown', 1, 5)).toBe(2);
  });
  it('jumps with Home and End', () => {
    expect(nextTab('Home', 3, 5)).toBe(0);
    expect(nextTab('End', 0, 5)).toBe(4);
  });
  it('ignores other keys', () => expect(nextTab('a', 0, 5)).toBeNull());
});
