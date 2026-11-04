/**
 * First run in under a minute: sign in with ChatGPT (which also makes this app's
 * Oya key when it has none: no Oya account to create), tell the server the model
 * lives here, connect and import the person's logins, and offer to become the
 * default browser. The same `connect` serves an already signed-in browser that
 * just switches its model to the ChatGPT plan. Without a ChatGPT plan,
 * `connectWithKey` does the same from a typed email and a Gemini API key.
 */
import type { AppServices } from '../app/services.ts';
import { DefaultBrowser } from '../app/default-browser.ts';
import { ServerApi } from '../connection/server-api.ts';
import { HOME_URL } from '../tabs/constants.ts';
import { signUp as agentSignUp, type SignedUp } from './signup.ts';
import * as C from './constants.ts';
import type { ChatgptAccount } from './auth.ts';
import { SetupAttempt } from './attempt.ts';
import { boundedSignal } from './oauth.ts';

/** The services onboarding uses. */
type Deps = Pick<AppServices, 'config' | 'socket' | 'mirror' | 'chatgpt' | 'tabs' | 'electron' | 'shell'>;

/** The project's model settings once its ChatGPT sign-in lives on this desktop: the server relays every call here. */
const DESKTOP_MODEL = {
  llm_provider: 'chatgpt',
  openai_api_key: C.DESKTOP_HELD,
  openai_base_url: null,
  chat_model: null,
};

/** The project's model settings for a Gemini key typed at first run; the server's catalog has Gemini's endpoint and model. */
const geminiModel = (key: string) => ({
  llm_provider: 'gemini',
  openai_api_key: key,
  openai_base_url: null,
  chat_model: null,
});

/** How the person asked to connect. */
export interface ConnectOptions {
  /** Bring in the default browser's logins once connected. */
  importLogins: boolean;
}

/** The key route's input: the email the Oya key is made from, and the Gemini key the project runs on. */
export interface KeyConnectOptions extends ConnectOptions {
  /** The person's email. */
  email: string;
  /** Their Gemini API key. */
  key: string;
}

/** What the checklist shows. */
export interface OnboardingStatus {
  /** The socket has authenticated this browser; a saved key alone is insufficient. */
  connected: boolean;
  /** This browser has an Oya key. */
  signedIn: boolean;
  /** The ChatGPT sign-in, and as whom. */
  chatgpt: ChatgptAccount;
  /** The project has a model: the ChatGPT plan, or a key given at first run. */
  hasModel: boolean;
  /** The person's logins came in from another browser. */
  imported: boolean;
  /** Oya Browser is the system's default browser. */
  isDefault: boolean;
  /** Where the person claims this browser's key into an Oya account, when it has none yet. */
  claimUrl: string;
}

/** The first-run flow and its state. */
export class Onboarding {
  /** True from a first-run connect until the person starts browsing: the checklist stays up meanwhile. */
  active = false;
  /** A ChatGPT sign-in that worked while a later step failed: a retry starts from it, not from OpenAI's page again. */
  private signedIn: ChatgptAccount | null = null;
  /** A single provider setup owns mutations until completion or cancellation. */
  private attempt: SetupAttempt | null = null;
  /** The main-process services. */
  private readonly deps: Deps;
  /** The server's HTTP API. */
  private readonly api: ServerApi;
  /** The system default-browser setting. */
  private readonly defaultBrowser: DefaultBrowser;
  /** The agent sign-up, replaceable in tests. */
  private readonly signUp: (serverUrl: string, email: string, signal?: AbortSignal) => Promise<SignedUp>;

  /** Runs over `deps`; `signUp` stands in for the server's agent sign-up in tests. */
  constructor(deps: Deps, signUp = agentSignUp) {
    this.deps = deps;
    this.api = new ServerApi(deps);
    this.defaultBrowser = new DefaultBrowser(deps);
    this.signUp = (url, email, signal) => signUp(url, email, undefined, signal);
  }

  /** Holds the welcome screen until the person explicitly starts browsing. */
  begin(): void {
    this.active = true;
  }

  /** Stops provider work while preserving the screen's hold across path changes. */
  cancel(): void {
    this.attempt?.abort.abort();
    this.deps.chatgpt.cancelSignIn?.();
    this.signedIn = null;
  }

  /** Explicit exit, sign-out and owner shutdown release the first-run hold. */
  reset(): void {
    this.cancel();
    this.active = false;
  }

  /** Enforces one provider operation and binds it to the selected workspace. */
  private async during<T>(work: (attempt: SetupAttempt) => Promise<T>): Promise<T> {
    if (this.attempt) throw Error('A connection is still finishing. Try again in a moment.');
    const attempt = (this.attempt = new SetupAttempt(() => this.deps.config.values));
    try {
      return await work(attempt);
    } finally {
      if (this.attempt === attempt) this.attempt = null;
    }
  }

  /** Signs in with ChatGPT and puts the project's model on it; a browser without a key gets one and connects. */
  async connect({ importLogins }: ConnectOptions): Promise<ChatgptAccount> {
    return this.during((attempt) => this.connectPlan(importLogins, attempt));
  }

  /** Advances the ChatGPT route only while its original workspace remains selected. */
  private async connectPlan(importLogins: boolean, attempt: SetupAttempt): Promise<ChatgptAccount> {
    const account = this.signedIn ?? (await this.signIn());
    attempt.check();
    this.signedIn = account;
    const minted = await this.ensureKey(account.email, attempt);
    await this.api.postCancelable('config', DESKTOP_MODEL, boundedSignal(attempt.abort.signal));
    attempt.check();
    this.configured(minted, importLogins);
    return account;
  }

  /** Successful configuration clears retry state and starts the first authenticated socket if needed. */
  private configured(minted: boolean, importLogins: boolean): void {
    this.signedIn = null;
    if (minted || !this.deps.socket.ready) this.connectFirstTime(importLogins);
  }

  /** The key route: makes this browser's Oya key from `email` when it has none and puts the project on the Gemini `key`. */
  async connectWithKey({ email, key, importLogins }: KeyConnectOptions): Promise<void> {
    const [mail, secret] = [String(email || '').trim(), String(key || '').trim()];
    if (!C.EMAIL_SHAPE.test(mail)) throw Error(C.BAD_EMAIL);
    if (!secret) throw Error(C.NO_GEMINI_KEY);
    return this.during((attempt) => this.connectKey(mail, secret, importLogins, attempt));
  }

  /** Applies a typed model key to the same workspace that started setup. */
  private async connectKey(mail: string, secret: string, importLogins: boolean, attempt: SetupAttempt): Promise<void> {
    const minted = await this.ensureKey(mail, attempt);
    await this.api.postCancelable('config', geminiModel(secret), boundedSignal(attempt.abort.signal));
    attempt.check();
    this.deps.config.merge({ modelFromKey: true });
    this.deps.config.save();
    this.configured(minted, importLogins);
  }

  /** Signs in to ChatGPT, then brings Oya back in front of the browser the sign-in ran in. */
  private async signIn(): Promise<ChatgptAccount> {
    const account = await this.deps.chatgpt.signIn();
    this.deps.shell.window?.show();
    return account;
  }

  /** Makes this browser's key from the ChatGPT email when it has none; answers whether it did. */
  private async ensureKey(email: string, attempt: SetupAttempt): Promise<boolean> {
    attempt.check();
    const { config } = this.deps;
    if (config.values.apiKey) return false;
    if (!email) throw Error(C.NO_EMAIL);
    const { apiKey, claimUrl } = await this.signUp(config.values.serverUrl, email, attempt.abort.signal);
    attempt.adoptKey(apiKey);
    this.keepKey(apiKey, claimUrl);
    return true;
  }

  /** Persists a new workspace credential only after its attempt has revalidated ownership. */
  private keepKey(apiKey: string, claimUrl: string): void {
    const config = this.deps.config;
    config.merge({ apiKey, claimUrl, keyFromApp: true, signedOut: false, account: null, modelFromKey: false });
    config.save();
  }

  /** Connects with the new key, holding the checklist up, and imports the logins once connected when asked. */
  private connectFirstTime(importLogins: boolean): void {
    this.active = true;
    this.deps.mirror.importOnConnect = importLogins;
    this.deps.socket.browserId = null;
    this.deps.socket.connect();
  }

  /** Where each step stands. */
  status(): OnboardingStatus {
    const { apiKey, mirroredFrom, imports, claimUrl, modelFromKey } = this.deps.config.values;
    const chatgpt = this.deps.chatgpt.status();
    return {
      connected: !!this.deps.socket.ready,
      ...{ signedIn: !!apiKey, chatgpt, hasModel: chatgpt.connected || !!modelFromKey },
      ...{ imported: !!mirroredFrom || (Array.isArray(imports) && imports.length > 0) },
      ...{ isDefault: this.defaultBrowser.isDefault(), claimUrl: String(claimUrl || '') },
    };
  }

  /** Done: the checklist goes and the browser opens on the start page. */
  finish(): void {
    this.reset();
    this.deps.tabs.enterBrowsingMode(HOME_URL);
  }
}
