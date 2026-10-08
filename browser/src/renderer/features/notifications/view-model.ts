/** Notification state and ordered native-view overlay ownership. */
import { ViewModel } from '../../core/view-model.ts';
import type { OyaBrowser } from '../../core/bridge.ts';
import type { BrowserNotification } from '../../../shared/notifications.ts';
import { NOTIFICATIONS_OVERLAY } from './constants.ts';
/** Only the trusted inbox and overlay channels are needed. */
type Bridge = Pick<
  OyaBrowser,
  | 'listNotifications'
  | 'readNotifications'
  | 'dismissNotification'
  | 'onNotificationsChanged'
  | 'onFingerprintChanged'
  | 'showOverlay'
  | 'hideOverlay'
>;
/** The toolbar and inbox share one snapshot. */
interface State {
  /** Newest first. */
  items: BrowserNotification[];
  /** Open only at the person's request. */
  open: boolean;
  /** Failure feedback without an unhandled promise. */
  error: string;
}
/** Incoming alerts update the bell, never focus or the page's visibility. */
export class NotificationsViewModel extends ViewModel<State> {
  /** IPC collaborator. */
  private readonly bridge: Bridge;
  /** Ignore stale reads after newer actions or disposal. */
  private revision = 0;
  /** Serialize native screenshot/show and hide. */
  private overlay = Promise.resolve();
  /** Subscribe before fetching so an early arrival cannot be lost. */
  constructor(bridge: Bridge) {
    super({ items: [], open: false, error: '' });
    this.bridge = bridge;
    this.own(bridge.onNotificationsChanged(() => void this.refresh()));
    this.own(bridge.onFingerprintChanged(() => this.profileChanged()));
    void this.refresh();
  }
  /** Count only unread entries, including arrivals while the inbox is open. */
  get unread(): number {
    return this.state.items.filter((item) => !item.read).length;
  }
  /** Fetch latest data without letting an older response overwrite it. */
  async refresh(): Promise<void> {
    const revision = ++this.revision;
    try {
      const items = await this.bridge.listNotifications();
      if (revision === this.revision) this.set({ items: Array.isArray(items) ? items : [], error: '' });
    } catch {
      if (revision === this.revision) this.set({ error: 'Could not load notifications. Try again.' });
    }
  }
  /** Opening is explicit; new arrivals never hide a website. */
  open(): void {
    if (this.state.open) return;
    this.set({ open: true });
    this.sync();
    void this.refresh();
  }
  /** Escape and the close button restore browsing. */
  close(): void {
    if (!this.state.open) return;
    this.set({ open: false });
    this.sync();
  }
  /** Mark read only the entries currently visible. */
  read(): Promise<void> {
    return this.change(() => this.bridge.readNotifications(this.state.items.map((item) => item.id)));
  }
  /** Remove one entry or the whole inbox. */
  dismiss(id?: string): Promise<void> {
    return this.change(() => this.bridge.dismissNotification(id));
  }
  /** Mutations report errors and then reload the authoritative state. */
  private async change(action: () => Promise<BrowserNotification[]>): Promise<void> {
    try {
      await action();
      await this.refresh();
    } catch {
      this.set({ error: 'Could not update notifications. Try again.' });
    }
  }
  /** A pending screenshot must finish before releasing this overlay. */
  private sync(): void {
    this.overlay = this.overlay
      .catch(() => {})
      .then(() => this.syncOverlay())
      .catch(() => {
        this.set({ error: 'Could not open notifications.' });
        this.close();
      });
  }
  /** Profile transitions clear text immediately, before the IPC response. */
  private profileChanged(): void {
    this.set({ items: [] });
    this.close();
    void this.refresh();
  }
  /** Native view changes are ordered separately from renderer state changes. */
  private async syncOverlay(): Promise<void> {
    if (this.state.open) await this.bridge.showOverlay(NOTIFICATIONS_OVERLAY);
    else await this.bridge.hideOverlay(NOTIFICATIONS_OVERLAY);
  }
  /** Disposed views cannot leave a hidden native page behind. */
  override dispose(): void {
    this.revision++;
    this.close();
    super.dispose();
  }
}
