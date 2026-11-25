/**
 * The first-run checklist's calls: connect ChatGPT or a Gemini key (and, with it,
 * an Oya key and the person's logins), read where each step stands, finish, and open the claim
 * page that turns the key into an account.
 */
import type { AppServices } from '../app/services.ts';
import type { HandlersOf } from './handle.ts';
import type { Payload, Failure, ConnectGeminiOptions } from '../../shared/ipc.ts';
import { HttpStatus } from '../connection/constants.ts';
import { GEMINI_KEYS_URL } from '../chatgpt/constants.ts';

/** A claim address the app may open: any https one, or http on this machine. */
const CLAIM_URL = /^(https:\/\/|http:\/\/(localhost|127\.0\.0\.1)(:\d+)?\/)/;

/** The services the checklist uses. */
type Deps = Pick<AppServices, 'onboarding' | 'config' | 'electron' | 'desktopSignIn' | 'modelSettings'>;

/** The channels this group answers. */
type Channel =
  | 'connect-chatgpt'
  | 'connect-gemini'
  | 'open-gemini-keys'
  | 'onboarding-status'
  | 'finish-onboarding'
  | 'open-claim'
  | 'begin-onboarding'
  | 'cancel-onboarding-attempt'
  | 'open-setup-help'
  | 'start-desktop-sign-in'
  | 'test-model-key'
  | 'begin-technical-model';

/** Only these documentation destinations can be opened from first-run guidance. */
const HELP = {
  'claude-install': 'https://code.claude.com/docs/en/setup',
  'claude-auth': 'https://code.claude.com/docs/en/authentication#generate-a-long-lived-token',
} as const;

/** The checklist's handlers. */
export class OnboardingHandlers {
  /** Channel → handler. */
  readonly handlers: HandlersOf<Channel> = {
    'begin-technical-model': (event) => this.deps.modelSettings.beginFlow(event.sender),
    'test-model-key': (event, provider) =>
      this.deps.modelSettings.test(event.sender, String(provider)).then(
        (answer) => answer as Payload,
        (error: DiagnosticError) => ({
          error: error.message,
          code: error.status === HttpStatus.NOT_FOUND ? 'model_test_unavailable' : error.code || 'model_unavailable',
        }),
      ),
    'start-desktop-sign-in': (event) =>
      this.deps.desktopSignIn.start(event.sender).then(
        () => ({ ok: true }),
        () => ({ error: 'Sign-in didn’t finish. Try again.' }),
      ),
    'connect-chatgpt': (_e, options) => this.connect(Boolean(options?.importLogins)),
    'connect-gemini': (_e, options) => this.connectGemini(options),
    'open-gemini-keys': () => this.deps.electron.shell.openExternal(GEMINI_KEYS_URL),
    'onboarding-status': () => this.deps.onboarding.status() as unknown as Payload,
    'begin-onboarding': () => this.deps.onboarding.begin(),
    'cancel-onboarding-attempt': (event) => {
      this.cancel(event.sender.id);
    },
    'open-setup-help': (_e, topic) => {
      if (Object.hasOwn(HELP, topic)) return this.deps.electron.shell.openExternal(HELP[topic]);
    },
    'finish-onboarding': (event) => {
      this.cancel(event.sender.id);
      this.deps.onboarding.finish();
    },
    'open-claim': () => this.openClaim(),
  };
  /** The main-process services. */
  private readonly deps: Deps;

  /** `deps` is the main-process services. */
  constructor(deps: Deps) {
    this.deps = deps;
  }
  /** A flow's owner cancels both credential writes and sign-in callbacks. */
  private cancel(owner: number): void {
    this.deps.desktopSignIn?.cancel(owner);
    this.deps.modelSettings?.cancel(owner);
    this.deps.onboarding.cancel();
  }

  /** Connects, answering `{ ok, email }` or the reason it could not. */
  private connect(importLogins: boolean): Promise<Payload & Failure> {
    return this.deps.onboarding.connect({ importLogins }).then(
      ({ email }) => ({ ok: true, email }),
      (e: Error) => ({ error: e.message }),
    );
  }

  /** Connects on a Gemini key, answering `{ ok }` or the reason it could not. */
  private connectGemini(options: ConnectGeminiOptions): Promise<Payload & Failure> {
    const { email, key, importLogins } = options || {};
    return this.deps.onboarding.connectWithKey({ email, key, importLogins: Boolean(importLogins) }).then(
      () => ({ ok: true }),
      (e: Error) => ({ error: e.message }),
    );
  }

  /** The claim page the server gave at sign-up: https, or plain http only on this machine (a local server). */
  private async openClaim(): Promise<void> {
    const url = String(this.deps.config.values.claimUrl || '');
    if (CLAIM_URL.test(url)) await this.deps.electron.shell.openExternal(url);
  }
}

/** Diagnostic response metadata contains recovery categories, never provider bodies. */
interface DiagnosticError extends Error {
  /** Stable recovery category. */ code?: string;
  /** Old servers may not implement diagnostics yet. */ status?: number;
}
