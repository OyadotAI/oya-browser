/** Window resource teardown never disconnects the application-wide profile or socket. */
import type { AppServices } from '../app/services.ts';
/** BrowserWindow closure destroys its native host, so detach page views before that point. */
export function closeWindowTabs(scope: AppServices): void {
  for (const tab of [...scope.tabs.list]) scope.tabs.closeTab(tab.id, { keepOne: false });
}
/** The per-window shield and its polling must not survive the native window. */
export function disposeWindow(scope: AppServices): void {
  scope.layout.flush();
  scope.shield.stopTracking();
  const contents = scope.shield.view?.webContents;
  if (contents && !contents.isDestroyed()) (contents as unknown as { destroy(): void }).destroy();
  scope.shield.view = null;
}
