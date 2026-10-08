/** A bounded, session-only notification inbox; changing profile discards its contents. */
import { randomUUID } from 'node:crypto';
import type { BrowserNotification } from '../../shared/notifications.ts';
import { NOTIFICATION_LIMIT, NOTIFICATION_MESSAGE_LIMIT } from './constants.ts';
/** Dependencies keep profile identity and delivery outside the inbox. */
interface Dependencies {
  /** Current cookie partition. */
  partition(): string;
  /** Tell the shell to refresh, without sending private text in events. */
  changed(): void;
}
/** Retain only a web origin, never a private meeting URL or credential. */
function notificationOrigin(value: string): string {
  try {
    const url = new URL(value);
    return ['http:', 'https:'].includes(url.protocol) ? url.origin : 'Page alert';
  } catch {
    return 'Page alert';
  }
}
/** Alerts never open a modal or steal focus. */
export class Notifications {
  /** Entries are discarded when the process ends. */
  private items: BrowserNotification[] = [];
  /** Last profile that used this inbox. */
  private partition = '';
  /** Profile and shell event collaborators. */
  private readonly deps: Dependencies;
  /** No disk or remote storage is involved. */
  constructor(deps: Dependencies) {
    this.deps = deps;
  }
  /** A profile change must not expose another profile's reminders. */
  list(): BrowserNotification[] {
    const partition = this.deps.partition();
    if (partition !== this.partition) {
      this.items = [];
      this.partition = partition;
    }
    return this.items.map((item) => ({ ...item }));
  }
  /** Bound both message size and entry count, newest first. */
  add(message: string, url = ''): void {
    this.list();
    const entry = notification(message, url);
    this.items = [entry, ...this.items].slice(0, NOTIFICATION_LIMIT);
    this.deps.changed();
  }
  /** Mark only the entries the person actually saw, not later arrivals. */
  read(ids: string[]): BrowserNotification[] {
    this.list();
    this.items = this.items.map((item) => (ids.includes(item.id) ? { ...item, read: true } : item));
    this.deps.changed();
    return this.list();
  }
  /** Remove an entry, or clear this profile's entire inbox. */
  dismiss(id?: string): BrowserNotification[] {
    this.list();
    this.items = id === undefined ? [] : this.items.filter((item) => item.id !== id);
    this.deps.changed();
    return this.list();
  }
}

/** Construct a safe, bounded, plain-text notification. */
function notification(message: string, url: string): BrowserNotification {
  return {
    id: randomUUID(),
    message: message.slice(0, NOTIFICATION_MESSAGE_LIMIT),
    source: notificationOrigin(url),
    time: Date.now(),
    read: false,
  };
}

export { listNotifications, markNotificationsRead, dismissNotification } from './agent.ts';
