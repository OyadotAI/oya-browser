/** Inbox tools run through normal authenticated browser command admission. */
import type { TabHandler } from './tab-commands.ts';
import { listNotifications, markNotificationsRead, dismissNotification } from '../notifications/index.ts';
/** These operations need no active page and never steal focus. */
export const NOTIFICATION_COMMANDS: Record<string, TabHandler> = {
  list_notifications: (runner, id, args) =>
    runner.sendResult(id, true, listNotifications(runner.deps.notifications, args)),
  mark_notifications_read: (runner, id, args) =>
    runner.sendResult(id, true, markNotificationsRead(runner.deps.notifications, args?.ids)),
  dismiss_notification: (runner, id, args) =>
    runner.sendResult(id, true, dismissNotification(runner.deps.notifications, args?.notification_id, args?.confirm)),
};
