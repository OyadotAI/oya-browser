/**
 * Normal and incognito windows. An incognito window's tabs live in the shared
 * throwaway session: they are never an application target, never listed to
 * agents or session restore, and never share a strip with a normal tab.
 */
import type { AppServices } from '../app/services.ts';

/** An incognito window. */
export const isPrivateScope = (scope: AppServices): boolean => !!scope.tabs.privateSession;

/** Normal windows, the only ones agents, session restore and root tab calls may reach. */
export const publicScopes = (scopes: AppServices[]): AppServices[] => scopes.filter((s) => !isPrivateScope(s));

/** Whether two windows hold the same session kind, so a tab may move between them. */
export const sameKind = (a: AppServices, b: AppServices): boolean => a.tabs.privateSession === b.tabs.privateSession;

/** A tab moved between kinds would carry one session into the other's strip. */
export function assertSameKind(source: AppServices, target: AppServices): void {
  if (!sameKind(source, target)) throw new Error('Tabs cannot move in or out of incognito');
}
