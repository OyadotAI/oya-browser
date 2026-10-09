/** Browser-scoped operations must explicitly select an owned ephemeral context. */
import type { NativeCommand } from './types.ts';
/** No download policy can silently change the person's default browser profile. */
export const BROWSER_PARAMS: Record<string, readonly string[]> = {
  'Browser.setDownloadBehavior': ['behavior', 'browserContextId', 'downloadPath', 'eventsEnabled'],
  'Browser.cancelDownload': ['guid', 'browserContextId'],
};
/** Reject unimplemented filename policies and missing context/destination fields before filesystem access. */
export function validateBrowser({ method, params }: NativeCommand): void {
  if (!Object.hasOwn(BROWSER_PARAMS, method)) return;
  requireBrowserString(params.browserContextId);
  if (method === 'Browser.cancelDownload') return requireBrowserString(params.guid);
  if (!['deny', 'allowAndName'].includes(String(params.behavior))) throw Error('Unsupported native download behavior');
  if (params.behavior === 'allowAndName') requireBrowserString(params.downloadPath);
  else if (params.downloadPath !== undefined) throw Error('A denied download cannot select a destination');
  if (params.eventsEnabled !== undefined && typeof params.eventsEnabled !== 'boolean')
    throw Error('eventsEnabled must be boolean');
}
/** Identifiers and paths must be nonempty strings; never coerce arbitrary values. */
function requireBrowserString(value: unknown): void {
  if (typeof value !== 'string' || !value) throw Error('A nonempty browser context, GUID or path is required');
}
