/** IPC: the dev panel's chat, page source, model settings and quick actions. */
import type { AppServices } from '../app/services.ts';
import type { PageSource as PageSourceAnswer, Payload } from '../../shared/ipc.ts';
import type { HandlersOf } from './handle.ts';
import { Chat, type ChatAnswer } from '../connection/chat.ts';
import { ServerApi } from '../connection/server-api.ts';
import { PageSource } from '../tabs/page-source.ts';
import { renderPage, FORMATS, type Analysis } from '../../page/render.ts';

/** The services the dev panel's handlers use. */
type Deps = Pick<
  AppServices,
  'config' | 'socket' | 'control' | 'shield' | 'chatAbort' | 'tabs' | 'protection' | 'world' | 'actions'
>;

/** The channels this group answers. */
type Channel =
  | 'model-status'
  | 'save-model-key'
  | 'send-chat'
  | 'stop-chat'
  | 'save-chat-playbook'
  | 'get-page-source'
  | 'render-page'
  | 'dev-action';

/** One model provider the server offers. */
interface Provider {
  /** Its id ('openai', 'anthropic'...). */
  id: string;
  /** Its name for people. */
  label?: string;
  /** Its API endpoint. */
  base?: string;
  /** Its default model. */
  model?: string;
  /** Its models. */
  models?: unknown[];
}

/** The model settings in effect. */
interface Effective {
  /** Whether the project has a model key. */
  hasLlmKey?: boolean;
  /** The model in use. */
  model?: string;
  /** The endpoint in use. */
  baseUrl?: string;
}

/** A kept analysis, as far as rendering it again goes. */
/** Whether a kept analysis still has the blocks a renderer needs. */
const isKeptAnalysis = (analysis: unknown): analysis is Analysis =>
  !!analysis && typeof analysis === 'object' && Array.isArray((analysis as Partial<Analysis>).blocks);

/** The project's model settings, as GET /config answers them. */
interface ServerConfig {
  /** The provider the key saved. */
  llm_provider?: string;
  /** The providers the server offers. */
  llm_catalog?: Provider[];
  /** What is in effect: whether there is a key, the model and the endpoint. */
  effective?: Effective;
}

/** A choice made in Ask: provider, model, and a key when one was typed. */
interface ModelChoice {
  /** The provider's id. */
  provider?: unknown;
  /** The model, or nothing for the provider's default. */
  model?: unknown;
  /** The key, when typed. */
  key?: unknown;
}

/**
 * What Ask offers against a server too old to send its catalog: the providers
 * it has always taken, each on its default model (any other is typed by hand).
 */
const LEGACY_CATALOG: Provider[] = [
  ['anthropic', 'Claude', 'sk-ant-...', 'https://console.anthropic.com/settings/keys', 'claude-opus-5'],
  ['openai', 'OpenAI', 'sk-...', 'https://platform.openai.com/api-keys', 'gpt-4o-mini'],
  ['gemini', 'Gemini', 'AIza...', 'https://aistudio.google.com/apikey', 'gemini-3.8-flash'],
].map(([id, label, hint, keysUrl, model]) => ({
  id,
  label,
  hint,
  keysUrl,
  model,
  models: [{ id: model, label: model }],
}));

/** The server's provider catalog, or the legacy one from a server that has none. */
const catalogOf = (config: ServerConfig): Provider[] =>
  config.llm_catalog?.length ? config.llm_catalog : LEGACY_CATALOG;

/** The provider a key runs on: the one it saved, else the catalog entry whose endpoint it is using. */
function currentProvider(config: ServerConfig): string {
  if (config.llm_provider) return config.llm_provider;
  return catalogOf(config).find((p) => p.base && p.base === config.effective?.baseUrl)?.id || '';
}

/** What the model card needs from the server's config. */
function statusOf(config: ServerConfig): Payload {
  const { hasLlmKey, model } = config.effective || {};
  return { hasLlmKey: !!hasLlmKey, provider: currentProvider(config), model, catalog: catalogOf(config) };
}

/**
 * The POST /config body for a choice made in Ask, or `{ error }`. The model is
 * always sent (never nulled behind the person's back), a key only when one was
 * typed, and the endpoint is reset only when the provider changes, so saving
 * here never undoes a model or gateway chosen in the console.
 */
export function modelUpdate(config: ServerConfig, { provider, model, key }: ModelChoice = {}): Payload {
  const secret = typeof key === 'string' ? key.trim() : '';
  if (!catalogOf(config).some((p) => p.id === provider)) return { error: 'Pick a provider.' };
  const changed = provider !== currentProvider(config);
  if (!secret && (changed || !config.effective?.hasLlmKey)) return { error: 'Paste your API key for this provider.' };
  const body: Payload = { llm_provider: provider, chat_model: (typeof model === 'string' && model.trim()) || null };
  if (secret) body.openai_api_key = secret;
  if (changed) body.openai_base_url = null;
  return body;
}

/** A kept analysis written again in a known format (the Source pane's format switch); '' otherwise. */
function renderKeptPage(analysis: unknown, format: string): string {
  if (!FORMATS.includes(format) || !isKeptAnalysis(analysis)) return '';
  return renderPage(analysis, format);
}

/** The dev panel's calls: Ask, the model card, the page source and quick actions. */
export class DevHandlers {
  /** Channel → handler. */
  readonly handlers: HandlersOf<Channel> = {
    'model-status': () => this.modelStatus(),
    'save-model-key': (_e, choice) => this.saveModelKey(choice),
    'send-chat': (_e, messages, data) => this.chat.send(messages, data),
    'stop-chat': () => this.chat.stop(),
    // The server keeps each browser's last run, so a body with only a name saves it as a playbook.
    'save-chat-playbook': (_e, name) => this.chat.ask('playbooks', { name }),
    // The html it reads is the page's outerHTML, a string.
    'get-page-source': () => new PageSource(this.deps).active() as Promise<PageSourceAnswer>,
    'render-page': (_e, analysis, format) => renderKeptPage(analysis, format),
    // The contract has no null: an action that answers nothing reaches the shell as null all the same.
    'dev-action': (_e, action, params) => this.deps.actions.runDevAction(action, params) as Promise<Payload>,
  };
  /** The main-process services. */
  private readonly deps: Deps;
  /** Ask, one run at a time. */
  private readonly chat: Chat;
  /** The server's API, as this browser. */
  private readonly api: ServerApi;

  /** `deps` is the main-process services (see src/main/main.ts). */
  constructor(deps: Deps) {
    this.deps = deps;
    this.chat = new Chat(deps);
    this.api = new ServerApi(deps);
  }

  /** The project's model settings, or null when the server cannot say. */
  private readConfig(): Promise<ServerConfig | null> {
    return this.api.get('config').catch(() => null) as Promise<ServerConfig | null>;
  }

  /** Whether this browser has a project, and the project's model as the server has it; unsure counts as having one. */
  private async modelStatus(): Promise<Payload> {
    const signedIn = !!this.deps.config.values.apiKey;
    if (!this.api.canCall()) return { signedIn, hasLlmKey: true };
    const config = await this.readConfig();
    return config ? { signedIn, ...statusOf(config) } : { signedIn, hasLlmKey: true };
  }

  /** Saves the provider, model and (when given) key chosen in Ask to the project, checked against what the server has now. */
  private async saveModelKey(choice: ModelChoice): Promise<ChatAnswer> {
    if (!this.api.canCall()) return { error: 'Not connected to server' };
    const config = await this.readConfig();
    if (!config) return { error: 'Could not read your project settings. Try again.' };
    const body = modelUpdate(config, choice);
    return body.error ? body : this.saveConfig(body);
  }

  /** Posts settings, as `{ ok }` or the server's reason. */
  private saveConfig(body: Payload): Promise<ChatAnswer> {
    return this.api.post('config', body).then(
      () => ({ ok: true }),
      (e: Error) => ({ error: e.message }),
    );
  }
}
