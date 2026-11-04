/** IPC: the dev panel's chat, page source, model settings and quick actions. */
import type { WebContents } from 'electron';
import type { AppServices } from '../app/services.ts';
import type { PageSource as PageSourceAnswer, Payload } from '../../shared/ipc.ts';
import type { HandlersOf } from './handle.ts';
import { Chat, type ChatAnswer } from '../connection/chat.ts';
import { ServerApi } from '../connection/server-api.ts';
import { statusOf, type ServerConfig, type ModelChoice } from '../connection/model-config.ts';
export { modelUpdate } from '../connection/model-config.ts';
import { PageSource } from '../tabs/page-source.ts';
import { renderPage, FORMATS, type Analysis } from '../../page/render.ts';

/** The services the dev panel's handlers use. */
type Deps = Pick<
  AppServices,
  | 'config'
  | 'socket'
  | 'control'
  | 'shield'
  | 'chatAbort'
  | 'chatRuns'
  | 'tabs'
  | 'protection'
  | 'world'
  | 'actions'
  | 'agentTab'
  | 'onboarding'
  | 'modelSettings'
>;

/** The channels this group answers. */
type Channel =
  | 'request-takeover'
  | 'model-status'
  | 'save-model-key'
  | 'sign-in-chatgpt'
  | 'send-chat'
  | 'stop-chat'
  | 'save-chat-playbook'
  | 'get-page-source'
  | 'render-page'
  | 'dev-action';

/** A kept analysis, as far as rendering it again goes. */
/** Whether a kept analysis still has the blocks a renderer needs. */
const isKeptAnalysis = (analysis: unknown): analysis is Analysis =>
  !!analysis && typeof analysis === 'object' && Array.isArray((analysis as Partial<Analysis>).blocks);

/** A kept analysis written again in a known format (the Source pane's format switch); '' otherwise. */
function renderKeptPage(analysis: unknown, format: string): string {
  if (!FORMATS.includes(format) || !isKeptAnalysis(analysis)) return '';
  return renderPage(analysis, format);
}

/** The dev panel's calls: Ask, the model card, the page source and quick actions. */
export class DevHandlers {
  /** Channel → handler. */
  readonly handlers: HandlersOf<Channel> = {
    'request-takeover': () => this.deps.shield.takeover.request(),
    'model-status': (event, requireCatalog) =>
      requireCatalog === true ? this.technicalStatus(event.sender) : this.modelStatus(),
    'save-model-key': (event, choice) => this.saveModelKey(choice, event.sender),
    'sign-in-chatgpt': () => this.signInChatgpt(),
    'send-chat': (_e, messages, data, tabId) => this.chat.send(messages, data, tabId),
    'stop-chat': (_e, tabId) => this.chat.stop(tabId),
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
    if (!this.api.canCall()) return { signedIn, error: 'Reconnect to load model settings.' };
    const config = await this.readConfig();
    return config
      ? { signedIn, ...statusOf(config) }
      : { signedIn, error: 'Could not load model settings. Try again.' };
  }
  /** A stalled Technical catalog can be cancelled and retried without retaining a native request. */
  private technicalStatus(owner: WebContents): Promise<Payload> {
    const signedIn = !!this.deps.config.values.apiKey;
    return this.deps.modelSettings.status(owner).then(
      (status) => ({ signedIn, ...status }),
      () => ({ signedIn, error: 'Could not load model settings. Try again.' }),
    );
  }

  /** Saves the provider, model and (when given) key chosen in Ask to the project, checked against what the server has now. */
  private saveModelKey(choice: ModelChoice, owner: WebContents): Promise<ChatAnswer> {
    return this.deps.modelSettings.write(owner, choice).then(
      () => ({ ok: true }),
      (error: Error) => ({ error: error.message }),
    );
  }

  /** Sign in with ChatGPT from the Ask card: this app holds the sign-in and the project's model moves to it. */
  private signInChatgpt(): Promise<ChatAnswer> {
    return this.deps.onboarding.connect({ importLogins: false }).then(
      () => ({ ok: true }),
      (e: Error) => ({ error: e.message }),
    );
  }
}
