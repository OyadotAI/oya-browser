/**
 * The answers `oya install --yes` uses: Docker on this machine, SQLite, one
 * Docker browser worker, and an LLM only when a key is already in the
 * environment. Agents run without a terminal, so this path asks nothing.
 */
import { LLM_PRESETS } from './llm.ts';
import type { Answers, Secrets } from './types.ts';
import { DEFAULT_PORT } from './constants.ts';

/** The default answers, and any secret the environment already supplies. */
export interface Defaults {
  /** What the wizard would otherwise ask. */
  answers: Answers;
  /** Credentials taken from the environment. */
  secrets: Secrets;
}

/** No LLM: agent control is added later with `oya config`. */
const NO_LLM: Answers['llm'] = { provider: 'skip', baseUrl: '', model: '' };

/** The LLM picked from the environment, and its key as the server reads it. */
interface EnvLlm {
  /** Which vendor, endpoint and model. */
  llm: Answers['llm'];
  /** OPENAI_API_KEY, when there is a key. */
  secrets: Secrets;
}

/** OPENAI_API_KEY, with its base URL and model when those are set too. */
function openaiFromEnv(env: NodeJS.ProcessEnv): EnvLlm {
  const { base, model } = LLM_PRESETS.openai;
  const llm = { provider: 'openai', baseUrl: env.OPENAI_BASE_URL || base, model: env.CHAT_MODEL || model };
  return { llm, secrets: { OPENAI_API_KEY: env.OPENAI_API_KEY || '' } };
}

/**
 * OPENAI_API_KEY first, since that is what the server itself reads; then
 * ANTHROPIC_API_KEY, written as OPENAI_API_KEY against Anthropic's endpoint;
 * otherwise none.
 */
function llmFromEnv(env: NodeJS.ProcessEnv): EnvLlm {
  if (env.OPENAI_API_KEY) return openaiFromEnv(env);
  if (!env.ANTHROPIC_API_KEY) return { llm: NO_LLM, secrets: {} };
  const { base, model } = LLM_PRESETS.anthropic;
  return { llm: { provider: 'anthropic', baseUrl: base, model }, secrets: { OPENAI_API_KEY: env.ANTHROPIC_API_KEY } };
}

/** Everything but the LLM: Docker here, SQLite, one worker, no optional services. */
const LOCAL: Omit<Answers, 'llm'> = {
  version: 1,
  host: 'docker',
  database: 'sqlite',
  fleet: 'docker-workers',
  workers: 1,
  publicUrl: `http://localhost:${DEFAULT_PORT}`,
  optional: { captcha: '', recordingBucket: '', metrics: false },
};

/** The whole default install, from the environment given. */
export function defaultInstall(env: NodeJS.ProcessEnv = process.env): Defaults {
  const { llm, secrets } = llmFromEnv(env);
  return { answers: { ...LOCAL, optional: { ...LOCAL.optional }, llm }, secrets };
}
