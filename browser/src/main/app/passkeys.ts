/** Native WebAuthn setup and explicit account selection; private keys stay in the OS. */
import type { App, Session, SelectWebauthnAccountDetails, WebContents, MessageBoxOptions } from 'electron';
import type { AppServices } from './services.ts';
import { PASSKEY_ACCOUNT_LIMIT, PASSKEY_LABEL_LIMIT, PASSKEY_PROMPT_TIMEOUT_MS, PASSKEY_DIALOG } from './constants.ts';

/** Native APIs and the browser's human-control boundary. */
type Deps = Pick<AppServices, 'electron' | 'tabs' | 'control' | 'governance'>;

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
  private readonly deps: Deps;
  /** Reusing a persistent session must not open duplicate dialogs. */
  private readonly installed = new WeakSet<Session>();
  /** At most one account chooser can own the UI. */
  private pending = false;
  /** Keep authentication outside the agent tools and renderer IPC. */
  constructor(deps: Deps) {
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
  /** Only a live, foreground human-controlled tab or tracked popup can choose. */
  private allowed(contents: WebContents): boolean {
    return (
      !contents.isDestroyed() &&
      !this.deps.governance.configuration &&
      this.deps.control.snapshot().interactive &&
      this.deps.tabs.getActiveView()?.webContents === contents
    );
  }
  /** Chromium validates the relying party; the chooser also fences the source document. */
  async choose(details: SelectWebauthnAccountDetails): Promise<string | undefined> {
    const frame = details.frame;
    const contents = frame && this.deps.electron.webContents.fromFrame(frame);
    if (!contents || !this.allowed(contents) || this.pending || !details.accounts.length) return;
    if (details.accounts.length > PASSKEY_ACCOUNT_LIMIT || !frame || frame.detached) return;
    this.pending = true;
    return this.prompt(contents, details).finally(() => {
      this.pending = false;
    });
  }
  /** An account choice cannot survive navigation, destruction or an abandoned prompt. */
  private async prompt(contents: WebContents, details: SelectWebauthnAccountDetails): Promise<string | undefined> {
    const controller = new AbortController();
    const cleanup = this.watch(contents, controller);
    try {
      return await this.ask(contents, details, controller.signal);
    } finally {
      cleanup();
    }
  }
  /** Install listeners only for this request and remove them on every completion path. */
  private watch(contents: WebContents, controller: AbortController): () => void {
    const cancel = (): void => controller.abort();
    const timer = setTimeout(cancel, PASSKEY_PROMPT_TIMEOUT_MS);
    contents.once('did-start-navigation', cancel);
    contents.once('destroyed', cancel);
    return () => this.unwatch(contents, cancel, timer);
  }
  /** A finished request leaves neither timers nor document listeners behind. */
  private unwatch(contents: WebContents, cancel: () => void, timer: ReturnType<typeof setTimeout>): void {
    clearTimeout(timer);
    contents.removeListener('did-start-navigation', cancel);
    contents.removeListener('destroyed', cancel);
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
  private async ask(
    contents: WebContents,
    details: SelectWebauthnAccountDetails,
    signal: AbortSignal,
  ): Promise<string | undefined> {
    const result = await this.show(contents, this.options(details, signal));
    if (signal.aborted || !this.allowed(contents) || details.frame?.detached) return;
    return details.accounts[result.response - 1]?.credentialId;
  }
  /** Attach account selection to its owning native window when available. */
  private show(contents: WebContents, options: MessageBoxOptions): Promise<Electron.MessageBoxReturnValue> {
    const parent = this.deps.electron.BrowserWindow.fromWebContents(contents);
    return parent
      ? this.deps.electron.dialog.showMessageBox(parent, options)
      : this.deps.electron.dialog.showMessageBox(options);
  }
}
