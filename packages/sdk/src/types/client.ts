/**
 * The options the `Oya` client is constructed with.
 */
import type { LlmProvider } from './config.js';

/** How to reach the control plane, and as whom. */
export interface OyaOptions {
  /** Defaults to OYA_API_KEY. */
  apiKey?: string;
  /** Defaults to OYA_BASE_URL, then https://oyabrowser.com. */
  baseUrl?: string;
  /** Per-request timeout. Navigation gets its own, longer budget. */
  timeoutMs?: number;
  /** A fetch to use instead of the global one, for tests, proxies or old runtimes. */
  fetch?: typeof globalThis.fetch;
  /**
   * The model behind `ask()`, `extract()`, runs and healed replays, for this client
   * only. Left out, the key's saved model is used (`oya init`, the console).
   */
  model?: ModelOptions;
}

/**
 * A model picked in code, sent with every call and never saved to the key, so the
 * console and the desktop app keep theirs.
 */
export interface ModelOptions {
  /** Whose model. */
  provider: LlmProvider;
  /** The model id, e.g. 'claude-sonnet-5'. The provider's default when left out. */
  name?: string;
  /**
   * Your own key for that provider: calls go to the provider's own endpoint and are
   * billed to you. Without one, `name` runs on the credential your key already uses
   * (its saved one, else the server's), which must be `provider`'s; the server's own
   * offers its priced models only.
   */
  apiKey?: string;
}

/** What `Oya.signup()` takes. */
export interface SignupOptions {
  /** The email of the person the agent works for. They get the claim link that unlocks cloud browsers. */
  email: string;
  /** Defaults to OYA_BASE_URL, then https://oyabrowser.com. */
  baseUrl?: string;
  /** Save the key to ~/.oya/config.json, where `new Oya()` finds it later. On by default; a key already saved there is never replaced. */
  save?: boolean;
  /** A fetch to use instead of the global one. */
  fetch?: typeof globalThis.fetch;
}

/** A new key from `Oya.signup()`. */
export interface Signup {
  /** The key. Shown once; keep it secret. */
  apiKey: string;
  /** Send this to the person: opening it while signed in to Oya claims the key and unlocks cloud browsers. */
  claimUrl: string;
  /** Whether cloud browsers work yet (false until the key is claimed). */
  cloudBrowsers: boolean;
  /** Whether the key was saved to ~/.oya/config.json. */
  saved: boolean;
}

/** What `oya.desktop.connect()` takes. */
export interface DesktopOptions {
  /** The persona (id or name) the desktop app signs in as, switching a running app that is on another one. Defaults to the key's own. */
  persona?: string;
  /** Open the pairing link on this machine (the default). Off, the link is only in the error when the app does not connect. */
  open?: boolean;
  /** How long to wait for the person to click Connect in the app. Five minutes by default. */
  timeoutMs?: number;
}
