/** Native WebAuthn setup and explicit account selection; private keys stay in the OS. */
import type { App, Session, SelectWebauthnAccountDetails, MessageBoxOptions } from 'electron';
import { PASSKEY_ACCOUNT_LIMIT, PASSKEY_LABEL_LIMIT, PASSKEY_DIALOG } from './constants.ts';
import { PasskeyRequest, passkeySource, type PasskeyDeps } from './passkey-request.ts';

/** Only a build with a matching signing entitlement can enable Touch ID. */
export function configurePasskeys(
  app: Pick<App, 'configureWebAuthn'>,
  group: string,
  platform = process.platform,
): void {
  if (platform !== 'darwin' || !/^[A-Z0-9]{10}\.ai\.oya\.browser\.webauthn$/.test(group)) return;
  app.configureWebAuthn({ touchID: { keychainAccessGroup: group } });
}

/** Credential names are untrusted display text, never commands or markup. */
function label(account: SelectWebauthnAccountDetails['accounts'][number], index: number): string {
  const text = account.name || account.displayName || `Account ${index + 1}`;
  return text.replace(/\p{Cc}/gu, ' ').slice(0, PASSKEY_LABEL_LIMIT);
}

/** Session listeners are installed once even when a persona reconnects. */
export class Passkeys {
  /** Services are injected so account selection is hermetic in tests. */
  private readonly deps: PasskeyDeps;
  /** Reusing a persistent session must not open duplicate dialogs. */
  private readonly installed = new WeakSet<Session>();
  /** At most one account chooser can own the UI. */
  private pending: PasskeyRequest | null = null;
  /** Keep authentication outside the agent tools and renderer IPC. */
  constructor(deps: PasskeyDeps) {
    this.deps = deps;
  }
  /** Electron cancels discoverable credentials unless an account is explicitly selected. */
  install(session: Session): void {
    if (this.installed.has(session)) return;
    this.installed.add(session);
    session.on('select-webauthn-account', (_event, details, callback) => {
      void this.choose(details).then(callback, () => callback());
    });
  }
  /** A control claim revokes the original chooser before later ownership can make it eligible again. */
  sync(): void {
    this.pending?.sync();
  }
  /** Chromium validates the relying party; the chooser also fences the source document. */
  async choose(details: SelectWebauthnAccountDetails): Promise<string | undefined> {
    const frame = details.frame;
    const contents = frame && this.deps.electron.webContents.fromFrame(frame);
    if (!contents || !frame || this.pending || !details.accounts.length) return;
    if (details.accounts.length > PASSKEY_ACCOUNT_LIMIT || frame.detached || frame.isDestroyed()) return;
    const source = passkeySource(this.deps, contents, frame);
    if (source) return this.prompt(new PasskeyRequest(this.deps, source), details);
  }
  /** An account choice cannot survive navigation, destruction or an abandoned prompt. */
  private async prompt(request: PasskeyRequest, details: SelectWebauthnAccountDetails): Promise<string | undefined> {
    this.pending = request;
    try {
      return await this.ask(request, details);
    } finally {
      request.dispose();
      this.pending = null;
    }
  }
  /** Native cancellation is the default; never silently select the first credential. */
  private options(details: SelectWebauthnAccountDetails, signal: AbortSignal): MessageBoxOptions {
    return {
      ...PASSKEY_DIALOG,
      detail: details.relyingPartyId,
      buttons: ['Cancel', ...details.accounts.map(label)],
      signal,
    };
  }
  /** The OS owns the dialog; nothing credential-shaped is sent to a renderer or agent. */
  private async ask(request: PasskeyRequest, details: SelectWebauthnAccountDetails): Promise<string | undefined> {
    const options = this.options(details, request.controller.signal);
    const result = await this.deps.electron.dialog.showMessageBox(request.source.window, options);
    if (!request.current()) return;
    return details.accounts[result.response - 1]?.credentialId;
  }
}
