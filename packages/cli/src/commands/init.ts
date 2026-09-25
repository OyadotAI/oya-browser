/**
 * `oya init`: onboarding against the current key, the LLM agents use, where
 * browsers run, and CAPTCHA solving, saved with one config call.
 */
import type { Oya } from '@oya-ai/browser';
import { resolved } from '../config.ts';
import { InputError, answered, ask, askSecret, choose } from '../prompt.ts';
import type { Flags } from '../args.ts';
import { client } from '../context.ts';
import { MODELS_SHOWN } from '../constants.ts';

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

/** One LLM provider as the server's catalog (GET /config llm_catalog) lists it. */
interface LlmChoice {
  /** The llm_provider value. */
  id: string;
  /** Its name. */
  label: string;
  /** The model a key runs on when it names none. */
  model: string;
  /** The models it offers. */
  models?: {
    /** The model id. */
    id: string;
  }[];
}

/** The parts of the current config onboarding reads. */
interface CurrentConfig {
  /** Every browser provider. */
  providers: ProviderInfo[];
  /** Whether this key has its own LLM credential. */
  has_openai_key: boolean;
  /** The configured model. */
  chat_model: string;
  /** The LLM provider saved, if any. */
  llm_provider?: string;
  /** Every LLM provider and its models; missing from servers older than the catalog. */
  llm_catalog?: LlmChoice[];
}

/** Settings to save, by config field. */
type Updates = Record<string, unknown>;

/** What `oya init` offers a server too old to send its catalog. */
const LEGACY_CATALOG: LlmChoice[] = [
  { id: 'anthropic', label: 'Claude (Anthropic)', model: 'claude-opus-5' },
  { id: 'openai', label: 'OpenAI', model: 'gpt-4o-mini' },
  { id: 'gemini', label: 'Gemini (Google)', model: 'gemini-3.8-flash' },
  { id: 'vertex', label: 'Gemini Enterprise (Vertex AI)', model: 'gemini-2.5-flash' },
];

/** The LLM providers to offer: the server's, the same list the console and desktop app show. */
const catalogOf = (current: CurrentConfig) => (current.llm_catalog?.length ? current.llm_catalog : LEGACY_CATALOG);

/** The LLM menu; the skip note depends on whether a key is already configured. */
const modelOptions = (current: CurrentConfig) => [
  ...catalogOf(current).map((p) => ({ id: p.id, label: p.label })),
  { id: 'skip', label: 'Skip', note: current.has_openai_key ? 'keep what is configured' : 'no agent control' },
];

/**
 * The settings for an LLM choice. The model is always sent, null for the
 * provider's default, so a previous provider's model is never left behind;
 * a changed provider also drops the previous endpoint.
 */
export function llmUpdates(current: CurrentConfig, provider: string, key: string, model: string): Updates {
  const updates: Updates = { llm_provider: provider, chat_model: model || null };
  if (key) updates.openai_api_key = key;
  if (provider !== current.llm_provider) updates.openai_base_url = null;
  return updates;
}

/** Names the first few models the provider offers, so a model id need not be looked up. */
function showModels(choice: LlmChoice): void {
  const ids = (choice.models || []).map((m) => m.id);
  const more = ids.length > MODELS_SHOWN ? ', …' : '';
  if (ids.length) console.log(`   Models: ${ids.slice(0, MODELS_SHOWN).join(', ')}${more}`);
}

/** Step 1: the LLM, its key and model. */
async function askModel(current: CurrentConfig, updates: Updates): Promise<void> {
  console.log('\n── 1. Your model ──');
  const llm = await choose('Which LLM should agents use?', modelOptions(current));
  const choice = catalogOf(current).find((p) => p.id === llm);
  if (choice) Object.assign(updates, await askModelDetails(current, choice));
}

/** The key and model for the chosen LLM, as settings. */
async function askModelDetails(current: CurrentConfig, choice: LlmChoice): Promise<Updates> {
  const key = await askSecret(`${choice.label} API key:`);
  showModels(choice);
  const model = await ask('Default model:', choice.model);
  return llmUpdates(current, choice.id, key, model);
}

/** What an unconfigured provider needs, as the menu says it: an address for your own Chrome, a key for a vendor. */
const needsNote = (p: ProviderInfo) =>
  p.needs.some((f) => f.endsWith('_url')) ? 'needs its ws:// address' : 'needs an API key';

/** The prompt for one field a provider needs: an address shown as typed, a key by the provider's name. */
function fieldLabel(p: ProviderInfo, field: string): string {
  if (field.endsWith('_url')) return 'Chrome DevTools address, such as ws://127.0.0.1:9222:';
  if (field.endsWith('_api_key')) return `${p.label} API key:`;
  return `${p.label} ${field.replace(/^[a-z]+_/, '').replace(/_/g, ' ')}:`;
}

/** The provider menu, noting which are configured. */
function providerOptions(current: CurrentConfig) {
  const note = (p: ProviderInfo) => (p.needs.length ? (p.configured ? 'configured' : needsNote(p)) : undefined);
  return current.providers.map((p) => ({ id: p.id, label: p.label, note: note(p) }));
}

/** Step 2: the browser provider, and the fields it needs. Returns the provider. */
async function askProvider(current: CurrentConfig, updates: Updates): Promise<string> {
  console.log('\n── 2. Where your browsers run ──');
  const provider = await choose('Browser provider:', providerOptions(current));
  updates.browser_provider = provider;
  const info = current.providers.find((p) => p.id === provider);
  for (const field of info?.needs || []) await askField(info!, field, updates);
  return provider;
}

/** Asks for one field; an address is shown as typed, a key is not. Nothing typed keeps what is there. */
async function askField(info: ProviderInfo, field: string, updates: Updates): Promise<void> {
  const label = fieldLabel(info, field);
  const value = field.endsWith('_url') ? await ask(label) : await askSecret(label);
  if (value) updates[field] = value;
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

/** What is said when the wizard ran with no terminal and no input: the defaults it walked past were never chosen. */
const NO_ANSWERS =
  'oya init had no input to read, so its defaults were never chosen. Nothing was saved. Run it in a terminal, or pipe the answers in.';

/** Saves the answers against the key and marks it onboarded, unless nobody answered: closed input must not overwrite a person's settings. */
async function saveAnswers(oya: Oya, updates: Updates): Promise<void> {
  if (!answered()) throw new InputError(NO_ANSWERS);
  updates.onboarded = 'true';
  await oya.config.set(updates);
}

/** Asks every step, then saves. */
async function interview(oya: Oya): Promise<string> {
  const current = await oya.config.get<CurrentConfig>();
  const updates: Updates = {};
  await askModel(current, updates);
  const provider = await askProvider(current, updates);
  await askCaptcha(updates);
  await saveAnswers(oya, updates);
  return provider;
}

/** `oya init`. */
export async function cmdInit(flags: Flags): Promise<void> {
  const provider = await interview(client(flags));
  console.log('\n✅ Saved against your API key.');
  signInHint(provider);
  console.log('\n   Then:  oya start && oya goto https://example.com');
}
