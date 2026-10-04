/**
 * Overlays the shell raises over the page (menus, dialogs). A BrowserView
 * always paints above the shell's own HTML, so the page view is taken away
 * while an overlay is up, and a screenshot of it stands in as the backdrop.
 */
import { randomUUID } from 'node:crypto';
import type { BrowserView, Rectangle } from 'electron';
import type { AppServices } from '../app/services.ts';
import { OVERLAYS, BACKDROP_WAIT_MS } from './constants.ts';

/** The services the overlays use. */
type Deps = Pick<AppServices, 'tabs' | 'shell' | 'shield' | 'layout'>;

/** A frozen page, as the shell paints it under an overlay. */
interface Backdrop {
  /** Names this backdrop, so the shell can say it is up. */
  token: string;
  /** The page's screenshot, as a data URL. */
  image: string;
  /** Where the page sat. */
  bounds: Rectangle;
}

/** The overlays up right now and the backdrops being painted. */
export class Overlays {
  /** Names of the overlays currently shown. */
  readonly names = new Set<string>();
  /** Backdrop token to the call that stops waiting for it, while the shell paints it. */
  readonly backdropWaiters = new Map<string, () => void>();
  /** The main-process services. */
  private readonly deps: Deps;

  /** `deps` is the main-process context (see src/main/main.ts). */
  constructor(deps: Deps) {
    this.deps = deps;
  }

  /** The active tab, when pages are being shown at all. */
  private shownView(): BrowserView | null {
    const view = this.deps.tabs.getShownView() as BrowserView | null; // a tab shown in the window is a BrowserView; popups have windows of their own
    return view && this.deps.shell.browsingMode ? view : null;
  }

  /** Raises an overlay: freeze the page as a backdrop, then take the view away. */
  async show(name = 'legacy'): Promise<void> {
    if (!OVERLAYS.includes(name)) return;
    // The start page is the shell's own, so it stays live under the overlay: no frozen backdrop, no view to take away.
    const view = this.shownView();
    if (!this.names.size && view) await this.freeze(view);
    this.names.add(name);
    if (view) this.deps.shell.window!.removeBrowserView(view);
    this.deps.shield.sync();
  }

  /** Hands the shell a screenshot of the page and waits (briefly) for it to be painted. */
  private async freeze(view: BrowserView): Promise<void> {
    try {
      const screenshot = await view.webContents.capturePage();
      const token = randomUUID();
      await this.paintBackdrop(token, { token, image: screenshot.toDataURL(), bounds: view.getBounds() });
    } catch {
      /* A crashed page has no frame to preserve; the reload status remains visible. */
    }
  }

  /** Sends the backdrop and resolves when the shell says it is up, or after a short wait. */
  private paintBackdrop(token: string, backdrop: Backdrop): Promise<void> {
    return new Promise((resolve) => {
      const timer = setTimeout(() => this.settle(token, resolve), BACKDROP_WAIT_MS);
      this.backdropWaiters.set(token, () => this.settle(token, resolve, timer));
      this.deps.shell.send('page-backdrop', backdrop);
    });
  }

  /** Stops waiting for one backdrop. */
  private settle(token: string, resolve: () => void, timer?: NodeJS.Timeout): void {
    clearTimeout(timer);
    this.backdropWaiters.delete(token);
    resolve();
  }

  /** The shell painted a backdrop. */
  backdropReady(token: string): void {
    this.backdropWaiters.get(token)?.();
  }

  /** Lowers an overlay; with none left, the page comes back. */
  hide(name = 'legacy'): void {
    this.names.delete(name);
    const view = this.shownView();
    if (view && !this.names.size) {
      this.deps.shell.window!.setBrowserView(view);
      this.deps.layout.layoutActiveTab();
    }
    if (!this.names.size) this.deps.shell.send('page-backdrop', null);
  }
}
