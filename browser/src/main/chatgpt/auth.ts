/**
 * This app's ChatGPT sign-in: it opens OpenAI's page in the person's own browser,
 * takes the code on a loopback address, keeps the credential sealed on this
 * machine, and hands out a live access token, refreshing it one request at a time
 * (each refresh rotates the refresh token, so two at once would race). The token
 * never leaves this machine except to OpenAI.
 */
import { randomUUID } from 'node:crypto';
import type { AppServices } from '../app/services.ts';
import type { ConfigValues } from '../app/config-store.ts';
import { SealedJson } from './credential-store.ts';
import { awaitCallback, type Callback } from './loopback.ts';
import * as O from './oauth.ts';
import * as C from './constants.ts';
import { openOutside } from './outside.ts';

/** The services sign-in uses. */
type Deps = Pick<AppServices, 'config' | 'electron'>;

/** What tests replace: the network, the listener, and where the sealed file goes. */
export interface AuthSeams {
  /** fetch, for the token endpoint. */
  fetch?: O.Fetch;
  /** Starts the loopback listener. */
  listen?: () => Promise<Callback>;
  /** The sealed credential. */
  store?: SealedJson<O.Credential>;
}

/** Whether this app holds a ChatGPT sign-in, and as whom. */
export interface ChatgptAccount {
  /** A sign-in is kept. */
  connected: boolean;
  /** The account's email, or ''. */
  email: string;
}

/** What a sign-in that came back for another request or another client says. */
const MISMATCH = 'ChatGPT sign-in did not match. Try again.';

/** What a sign-in OpenAI's page refused shows; OpenAI's own code goes to the log, for support. */
function cancelled(code: string): Error {
  console.warn('[chatgpt] sign-in refused:', code);
  return Error(C.SIGN_IN_CANCELLED);
}

/** The ChatGPT account this app is signed in to. */
export class ChatgptAuth {
  /** The main-process services. */
  private readonly deps: Deps;
  /** The network. */
  private readonly fetch: O.Fetch;
  /** The loopback listener. */
  private readonly listen: () => Promise<Callback>;
  /** The credential at rest. */
  private readonly store: SealedJson<O.Credential>;
  /** The refresh in flight, so concurrent callers share one. */
  private refreshing: Promise<O.Credential> | null = null;

  /** Signs in through `deps`; `seams` stand in for the network and disk in tests. */
  constructor(deps: Deps, seams: AuthSeams = {}) {
    this.deps = deps;
    this.fetch = seams.fetch ?? fetch;
    this.listen = seams.listen ?? (() => awaitCallback());
    const dir = deps.electron.app.getPath('userData');
    this.store = seams.store ?? new SealedJson(dir, C.CREDENTIAL_FILE, deps.electron.safeStorage);
  }

  /** Signs in: OpenAI's page in the person's usual browser (never Oya itself), then the code exchanged and sealed. Answers the account. */
  async signIn(): Promise<ChatgptAccount> {
    const callback = await this.listen();
    try {
      const credential = await this.through(callback);
      this.store.save(credential);
      return { connected: true, email: credential.email || '' };
    } finally {
      callback.close();
    }
  }

  /** Opens the sign-in for `callback`'s address and waits for its credential. */
  private async through(callback: Callback): Promise<O.Credential> {
    // OpenAI's sign-in wants one host id per install and the client it issued reused: registering
    // afresh after a refused exchange is refused again, so a failed sign-in could never recover.
    const login = O.newLogin(callback.redirectUri, this.saved('chatgptClientId'));
    await openOutside(this.deps, O.authorizeUrl(login, this.hostId()));
    return this.complete(login, await callback.done);
  }

  /** Checks what came back, keeps the issued client before exchanging the code, never under the registration client. */
  private async complete(login: O.Login, query: Record<string, string>): Promise<O.Credential> {
    if (query.state !== login.state) throw Error(MISMATCH);
    if (query.error) throw cancelled(query.error);
    const clientId = query.client_id || login.clientId;
    if (clientId === C.DYNAMIC_CLIENT) throw Error('ChatGPT did not finish the sign-in. Try again.');
    if (login.clientId !== C.DYNAMIC_CLIENT && clientId !== login.clientId) throw Error(MISMATCH);
    this.keep({ chatgptClientId: clientId });
    const tokens = await O.exchangeCode(login, clientId, query.code || '', this.fetch);
    return O.credentialFrom(tokens, clientId, login.nonce);
  }

  /** A live access token, refreshed when it is close to expiring; throws when there is no sign-in. */
  async accessToken(): Promise<string> {
    const credential = this.store.load();
    if (!credential) throw Error('Not signed in to ChatGPT');
    if (credential.access_token && (credential.expires_at || 0) - Date.now() > C.REFRESH_EARLY_MS)
      return credential.access_token;
    return String((await this.refresh(credential)).access_token);
  }

  /** One refresh at a time; the rotated credential is sealed before anyone uses it. */
  private refresh(credential: O.Credential): Promise<O.Credential> {
    this.refreshing ??= O.refreshed(credential, this.fetch)
      .then((next) => (this.store.save(next), next))
      .finally(() => (this.refreshing = null));
    return this.refreshing;
  }

  /** Whether this app is signed in to ChatGPT, and as whom. */
  status(): ChatgptAccount {
    const credential = this.store.load();
    return { connected: !!credential, email: credential?.email || '' };
  }

  /** Forgets the sign-in. */
  signOut(): void {
    this.store.clear();
  }

  /** This install's agent host id: made once, kept, and carrying nothing about its owner. */
  private hostId(): string {
    const kept = this.saved('chatgptHostId');
    if (kept) return kept;
    const made = `urn:uuid:${randomUUID()}`;
    this.keep({ chatgptHostId: made });
    return made;
  }

  /** A kept registration value, or undefined when the config file holds anything but text there. */
  private saved(field: 'chatgptHostId' | 'chatgptClientId'): string | undefined {
    const value: unknown = this.deps.config.values[field];
    return typeof value === 'string' && value ? value : undefined;
  }

  /** Saves part of this install's ChatGPT registration to the config. */
  private keep(values: Pick<ConfigValues, 'chatgptHostId' | 'chatgptClientId'>): void {
    this.deps.config.merge(values);
    this.deps.config.save();
  }
}
