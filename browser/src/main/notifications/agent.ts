/** Validated, bounded agent access to the same inbox used by the toolbar. */
import type { BrowserNotification } from '../../shared/notifications.ts';
import type { Notifications } from './index.ts';
import {
  NOTIFICATION_LIMIT,
  NOTIFICATION_PAGE_LIMIT,
  NOTIFICATION_PAGE_DEFAULT,
  NOTIFICATION_ID_LENGTH,
} from './constants.ts';
/** The remote boundary accepts unknown values until validated. */
export interface NotificationQuery {
  /** Filter without marking anything read. */
  unread_only?: unknown;
  /** Maximum returned entries. */
  limit?: unknown;
  /** Matching entries to skip. */
  offset?: unknown;
}
/** Refuse invalid pagination instead of silently coercing it. */
function notificationPageNumber(value: unknown, fallback: number, min: number, max: number): number {
  if (value === undefined) return fallback;
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < min || value > max)
    throw new Error('Invalid notification pagination');
  return value;
}
/** Reading the inbox never marks entries read or opens a modal. */
export function listNotifications(inbox: Notifications, args: NotificationQuery = {}) {
  if (args.unread_only !== undefined && typeof args.unread_only !== 'boolean')
    throw new Error('unread_only must be boolean');
  const limit = notificationPageNumber(args.limit, NOTIFICATION_PAGE_DEFAULT, 1, NOTIFICATION_PAGE_LIMIT);
  const offset = notificationPageNumber(args.offset, 0, 0, Number.MAX_SAFE_INTEGER);
  const all = inbox.list(),
    unread = all.filter((item) => !item.read);
  const matches = args.unread_only ? unread : all;
  return notificationPage(matches, offset, limit, unread.length);
}
/** Structured pages keep read results bounded and provide a continuation offset. */
function notificationPage(matches: BrowserNotification[], offset: number, limit: number, unread: number) {
  const entries = matches.slice(offset, offset + limit);
  return {
    entries,
    total: matches.length,
    unread_count: unread,
    next_offset: offset + entries.length < matches.length ? offset + entries.length : null,
    session_only: true,
  };
}
/** IDs, not message text, identify an entry for retry-safe mutations. */
function notificationId(value: unknown): string {
  if (typeof value !== 'string' || !value.trim() || value.length > NOTIFICATION_ID_LENGTH)
    throw new Error('Invalid notification id');
  return value;
}
/** Only explicitly selected entries are marked read; later arrivals stay unread. */
export function markNotificationsRead(inbox: Notifications, ids: unknown) {
  if (!Array.isArray(ids) || !ids.length || ids.length > NOTIFICATION_LIMIT)
    throw new Error('Provide 1–100 notification ids');
  const selected = ids.map(notificationId);
  const marked = inbox.list().filter((item) => !item.read && selected.includes(item.id)).length;
  inbox.read(selected);
  return { marked_read: marked };
}
/** A missing id must never become the UI's clear-all operation. */
export function dismissNotification(inbox: Notifications, id: unknown, confirm: unknown) {
  const selected = notificationId(id);
  if (confirm !== true) throw new Error('Dismissing a notification requires confirm: true after a user request');
  const removed = inbox.list().some((item) => item.id === selected);
  inbox.dismiss(selected);
  return { removed };
}
