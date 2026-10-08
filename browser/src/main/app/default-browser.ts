/** Explicit, human-initiated default-browser setup; never silently takes web associations. */
import type { AppServices } from './services.ts';

/** Only native OS integration and a parent window are needed. */
type Deps = Pick<AppServices, 'electron' | 'shell'>;
/** Both web schemes must agree before reporting success. */
const WEB_SCHEMES = ['http', 'https'];
/** Owns the OS interaction, not the installation or any browsing data. */
export class DefaultBrowser {
  /** Native APIs and the shell. */
  private readonly deps: Deps;
  /** Platform is injectable so tests never change the host's default. */
  private readonly platform: string;
  /** Capture the native seam without importing Electron at runtime. */
  constructor(deps: Deps, platform = process.platform) {
    this.deps = deps;
    this.platform = platform;
  }
  /** A verified status, not the return value of a registration request. */
  isDefault(): boolean {
    return WEB_SCHEMES.every((scheme) => this.deps.electron.app.isDefaultProtocolClient(scheme));
  }
  /** A click may launch OS confirmation; installation alone never changes the default. */
  async request(): Promise<void> {
    await this.requestInstalled().catch(() =>
      this.message(
        'Default browser was not changed',
        'Open your system’s default-app settings and select Oya Browser for HTTP and HTTPS links.',
      ),
    );
  }
  /** Development builds must not register their temporary executable with the OS. */
  private async requestInstalled(): Promise<void> {
    if (!this.deps.electron.app.isPackaged)
      return this.message(
        'Install Oya Browser first',
        'Use this option in the installed app, not the development browser.',
      );
    if (this.isDefault())
      return this.message('Oya is your default browser', 'HTTP and HTTPS links already open in Oya Browser.');
    await this.chooseDefault();
  }
  /** Windows requires the person to choose in Settings; other desktops use their protocol chooser. */
  private async chooseDefault(): Promise<void> {
    if (this.platform === 'win32') return this.deps.electron.shell.openExternal('ms-settings:defaultapps');
    for (const scheme of WEB_SCHEMES) this.deps.electron.app.setAsDefaultProtocolClient(scheme);
    if (this.isDefault())
      return this.message('Oya is your default browser', 'HTTP and HTTPS links will open in Oya Browser.');
    await this.message(
      'Finish in system settings',
      'Choose Oya Browser as your default web browser in system settings. On Linux, install the application’s desktop entry first. You can run this option again to check the result.',
    );
  }
  /** Native feedback stays visible above embedded browser views. */
  private async message(message: string, detail: string): Promise<void> {
    const options = { type: 'info' as const, message, detail, buttons: ['OK'] };
    const win = this.deps.shell.window;
    if (win) await this.deps.electron.dialog.showMessageBox(win, options);
    else await this.deps.electron.dialog.showMessageBox(options);
  }
}
