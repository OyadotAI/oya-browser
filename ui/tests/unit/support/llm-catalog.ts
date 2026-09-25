/**
 * A model catalog shaped as GET /config returns it (llm_catalog), for tests of
 * the pickers built from it: a few providers, each with a couple of models.
 */
import type { LlmPreset } from '@/components/dashboard/config';

/** The catalog: the providers onboarding offers, Gemini Enterprise, and their models. */
export const LLM_CATALOG: LlmPreset[] = [
  ['openai', 'OpenAI', 'gpt-4o-mini', 'sk-...', ['gpt-4o-mini', 'gpt-4.1']],
  ['anthropic', 'Claude', 'claude-opus-5', 'sk-ant-...', ['claude-opus-5', 'claude-sonnet-5']],
  ['gemini', 'Gemini', 'gemini-3.8-flash', 'AIza...', ['gemini-3.8-flash']],
  ['vertex', 'Gemini Enterprise', 'gemini-2.5-flash', 'AIza... (express mode)', ['gemini-2.5-flash']],
  [
    'openrouter',
    'OpenRouter',
    'anthropic/claude-sonnet-5',
    'sk-or-...',
    ['anthropic/claude-sonnet-5', 'x-ai/grok-4.7'],
  ],
].map(([id, label, model, hint, ids]) => ({
  id: id as string,
  label: label as string,
  model: model as string,
  hint: hint as string,
  keysUrl: `https://keys.test/${id}`,
  base: `https://${id}.test/v1`,
  models: (ids as string[]).map((m) => ({ id: m, label: m })),
}));
