/** A quiet toolbar bell and a keyboard-accessible, session-only inbox. */
import { useViewModel } from '../../hooks/index.ts';
import { Button, Dialog, IconButton } from '../../ui/index.ts';
import type { NotificationsViewModel } from './view-model.ts';
import type { BrowserNotification } from '../../../shared/notifications.ts';
import './notifications.css';
/** The same model drives the bell and dialog. */
interface Props {
  /** Inbox state and actions. */
  vm: NotificationsViewModel;
}
/** Badge changes are announced without stealing focus or opening a modal. */
export function NotificationsButton({ vm }: Props) {
  useViewModel(vm);
  return (
    <div className="notifications-button">
      <IconButton
        className="nav-btn"
        icon="bell"
        aria-label={`Notifications, ${vm.unread} unread`}
        aria-haspopup="dialog"
        title="Notifications (⌘/Ctrl Alt J)"
        onClick={() => vm.open()}
      />
      {vm.unread > 0 && (
        <span className="notifications-badge" aria-hidden="true">
          {vm.unread}
        </span>
      )}
      <span className="notifications-announcement" role="status">
        {vm.unread > 0 ? `${vm.unread} unread notifications` : ''}
      </span>
    </div>
  );
}
/** Plain text only: alerts cannot inject markup or clickable credential-bearing links. */
function Entry({ item, vm }: Props & { /** A retained notification. */ item: BrowserNotification }) {
  return (
    <li className={item.read ? '' : 'unread'}>
      <div className="notification-meta">
        <strong>{item.source}</strong>
        <time dateTime={new Date(item.time).toISOString()}>{new Date(item.time).toLocaleString()}</time>
      </div>
      <p>{item.message}</p>
      <Button onClick={() => void vm.dismiss(item.id)} aria-label={`Dismiss notification from ${item.source}`}>
        Dismiss
      </Button>
    </li>
  );
}
/** Shared Dialog traps focus, restores it on close, and supports Escape. */
export function NotificationsCenter({ vm }: Props) {
  const { items, open, error } = useViewModel(vm);
  return (
    <Dialog
      open={open}
      hidden={!open}
      onClose={() => vm.close()}
      focusId="notifications-close"
      id="notifications-overlay"
      className="notifications-overlay"
      label="Notifications"
    >
      <section className="notifications-panel">
        <InboxHeader vm={vm} />
        <div className="notifications-actions">
          <Button disabled={!vm.unread} onClick={() => void vm.read()}>
            Mark all read
          </Button>
          <Button disabled={!items.length} onClick={() => void vm.dismiss()}>
            Clear all
          </Button>
        </div>
        {error && (
          <p role="alert">
            {error} <Button onClick={() => void vm.refresh()}>Retry</Button>
          </p>
        )}
        {items.length ? (
          <ul>
            {items.map((item) => (
              <Entry key={item.id} item={item} vm={vm} />
            ))}
          </ul>
        ) : (
          <div className="notifications-empty">
            <h3>All caught up</h3>
            <p>Page reminders appear here without interrupting your browsing.</p>
          </div>
        )}
      </section>
    </Dialog>
  );
}

/** Stable focus target for keyboard users entering the inbox. */
function InboxHeader({ vm }: Props) {
  return (
    <header>
      <div>
        <h2>Notifications</h2>
        <p>Recent alerts · this session only</p>
      </div>
      <IconButton
        id="notifications-close"
        className="nav-btn"
        icon="close"
        aria-label="Close notifications"
        onClick={() => vm.close()}
      />
    </header>
  );
}
