/**
 * `oya init`: onboarding against the current key, the LLM agents use, where
 * browsers run, and CAPTCHA solving, saved with one config call.
 */
import type { Oya } from '@oya-ai/browser';
import { resolved } from '../config.ts';
import { ask, askSecret, choose } from '../prompt.ts';
import type { Flags } from '../args.ts';
import { client } from '../context.ts';

/** A browser provider as `config.get()` lists it. */
interface ProviderInfo {
  /** The provider's id. */
  id: string;
  /** Its display name. */
  label: string;
  /** Whether its fields are set. */
  configured: boolean;
  /** The config fields it requires. */
  needs: string[];
}

/** The parts of the current config onboarding reads. */
interface CurrentConfig {
  /** Every browser provider. */
  providers: ProviderInfo[];
  /** Whether this key has its own LLM credential. */
  has_openai_key: boolean;
  /** The configured model. */
  chat_model: string;
}

/** Settings to save, by config field. */
type Updates = Record<string, unknown>;

/** An LLM's display name and default model. */
interface Preset {
  /** Display name. */
  name: string;
  /** Default model. */
  model: string;
}

/** Each LLM's name and default model. */
const PRESETS: Record<string, Preset> = {
  anthropic: { name: 'Anthropic', model: 'claude-sonnet-5' },
  openai: { name: 'OpenAI', model: 'gpt-4o-mini' },
  gemini: { name: 'Gemini', model: 'gemini-3.8-flash' },
  vertex: { name: 'Gemini Enterprise', model: 'gemini-2.5-flash' },
};

/** The LLM menu; the skip note depends on whether a key is already configured. */
const modelOptions = (current: CurrentConfig) => [
  { id: 'anthropic', label: 'Claude (Anthropic)' },
  { id: 'openai', label: 'OpenAI' },
  { id: 'gemini', label: 'Gemini (Google)' },
  { id: 'vertex', label: 'Gemini Enterprise (Vertex AI)' },
  { id: 'skip', label: 'Skip', note: current.has_openai_key ? 'keep what is configured' : 'no agent control' },
];

/** Step 1: the LLM, its key and model. */
async function askModel(current: CurrentConfig, updates: Updates): Promise<void> {
  console.log('\n── 1. Your model ──');
  const llm = await choose('Which LLM should agents use?', modelOptions(current));
  if (llm !== 'skip') await askModelDetails(llm, updates);
}

/** The key and model for the chosen LLM. */
async function askModelDetails(llm: string, updates: Updates): Promise<void> {
  updates.llm_provider = llm;
  const key = await askSecret(`${PRESETS[llm].name} API key:`);
  if (key) updates.openai_api_key = key;
  const model = await ask('Default model:', PRESETS[llm].model);
  if (model) updates.chat_model = model;
}

/** The provider menu, noting which are configured. */
function providerOptions(current: CurrentConfig) {
  const note = (p: ProviderInfo) => (p.needs.length ? (p.configured ? 'configured' : 'needs an API key') : undefined);
  return current.providers.map((p) => ({ id: p.id, label: p.label, note: note(p) }));
}

/** Step 2: the browser provider, and the fields it needs. Returns the provider. */
async function askProvider(current: CurrentConfig, updates: Updates): Promise<string> {
  console.log('\n── 2. Where your browsers run ──');
  const provider = await choose('Browser provider:', providerOptions(current));
  updates.browser_provider = provider;
  for (const field of current.providers.find((p) => p.id === provider)?.needs || []) {
    const value = await askSecret(`${field.replace(/_/g, ' ')}:`);
    if (value) updates[field] = value;
  }
  return provider;
}

/** The CAPTCHA menu. */
const CAPTCHA_OPTIONS = [
  { id: '', label: 'No', note: 'providers that solve natively still will' },
  { id: 'capsolver', label: 'Yes, via CapSolver' },
];

/** Step 3: automatic CAPTCHA solving. */
async function askCaptcha(updates: Updates): Promise<void> {
  console.log('\n── 3. CAPTCHAs ──');
  const solver = await choose('Solve CAPTCHAs automatically?', CAPTCHA_OPTIONS);
  updates.captcha_solver = solver;
  if (!solver) return;
  const key = await askSecret('Solver API key:');
  if (key) updates.captcha_api_key = key;
}

/** Oya-run browsers inherit cookies from a desktop sign-in, so point there. */
function signInHint(provider: string): void {
  if (provider !== 'oya-cloud' && provider !== 'oya-selfhosted') return;
  console.log('\n── 4. Sign in once, on your own machine ──');
  console.log('   Your remote browsers reuse the cookies from a desktop sign-in, so agents');
  console.log('   arrive already logged in, as the same identity, from the same fingerprint.');
  console.log(`   Download the desktop browser: ${resolved().baseUrl}/downloads`);
}

/** Asks every step, then saves. */
async function interview(oya: Oya): Promise<string> {
  const current = await oya.config.get<CurrentConfig>();
  const updates: Updates = {};
  await askModel(current, updates);
  const provider = await askProvider(current, updates);
  await askCaptcha(updates);
  updates.onboarded = 'true';
  await oya.config.set(updates);
  return provider;
}

/** `oya init`. */
export async function cmdInit(flags: Flags): Promise<void> {
  const provider = await interview(client(flags));
  console.log('\n✅ Saved against your API key.');
  signInHint(provider);
  console.log('\n   Then:  oya start && oya goto https://example.com');
}
