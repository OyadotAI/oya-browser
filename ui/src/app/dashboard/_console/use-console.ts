/**
 * The console page's state, composed: the credential and project in force,
 * the view, the data feeds and their polling, the side effects on load, and
 * the actions and shortcuts. The page and its parts render from the result.
 */
'use client';

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useToast } from '@/components/dashboard/toast';
import { useShortcuts } from '@/lib/shortcuts';
import { usePatchState } from '@/lib/hooks/use-patch-state';
import { browsersOf, handleSlackReturn, pruneSelection, stopBrowsers, takeScreenshot, type Toast } from './actions';
import type { FleetFilter } from '@/components/dashboard/fleet-strip';
import type { Liveness } from './feeds';
import { savedProject, startPolling } from './polling';
import { buildShortcuts } from './shortcuts';
import { useBrowsers, useFleet, useKeyConfig, useLiveness, usePersonas } from './use-feeds';
import { INITIAL_VIEW, projectReset, type ConsoleView } from './view';

/** The project the credential opens. Credentials renew hourly; only a change of project resets the console. */
function useProject() {
  const [apiKey, setApiKey] = useState('');
  const [project, setProject] = useState<string | null>(null);
  const projectRef = useRef<string | null>(null);
  const handle = useMemo(() => ({ setApiKey, setProject, projectRef }), []);
  return { apiKey, project, handle };
}

/** The project handle: setters and the current project id. */
type ProjectHandle = ReturnType<typeof useProject>['handle'];

/** Credential, project, view and the input refs. */
function useConsoleBase() {
  const live = useLiveness();
  const { apiKey, project, handle } = useProject();
  const [view, patch] = usePatchState<ConsoleView>(INITIAL_VIEW);
  const filterRef = useRef<HTMLInputElement>(null);
  const urlRef = useRef<HTMLInputElement>(null);
  return { live, apiKey, project, handle, view, patch, filterRef, urlRef };
}

/** The console's base state. */
type Base = ReturnType<typeof useConsoleBase>;

/** Every feed, keyed to the credential and project in force. */
function useData({ apiKey, project, live, patch }: Base) {
  const setOnboarding = useCallback((show: boolean) => patch({ showOnboarding: show }), [patch]);
  const browsers = useBrowsers(apiKey, live);
  const fleet = useFleet(apiKey, live);
  const personas = usePersonas(apiKey, live);
  const config = useKeyConfig(apiKey, project, live, setOnboarding);
  return { ...browsers, ...fleet, ...personas, ...config };
}

/** The console's data. */
type Data = ReturnType<typeof useData>;

/** Adopts a credential; a different project also clears everything shown for the last one. */
function switchProject(h: ProjectHandle, live: Liveness, reset: () => void, credential: string, id: string | null) {
  // The credential in force now, so a response for a previous project is dropped instead of painted over the new one.
  live.keyRef.current = credential;
  h.setApiKey(credential);
  if (h.projectRef.current === id) return;
  h.projectRef.current = id;
  h.setProject(id);
  reset();
}

/** Clears every feed and the project's view state. */
function useProjectReset({ patch }: Base, { resetBrowsers, resetFleet, resetPersonas, resetConfig }: Data) {
  return useCallback(
    () => (resetBrowsers(), resetFleet(), resetPersonas(), resetConfig(), patch(projectReset())),
    [resetBrowsers, resetFleet, resetPersonas, resetConfig, patch],
  );
}

/** A stable openProject(credential, projectId). */
function useOpenProject(b: Base, d: Data) {
  const { handle, live } = b;
  const reset = useProjectReset(b, d);
  return useCallback(
    (cred: string, id: string | null) => switchProject(handle, live, reset, cred, id),
    [handle, live, reset],
  );
}

/** Reopens this tab's project and selected browser once, on load. */
function useRestore(openProject: (cred: string, id: string | null) => void, b: Base) {
  const { patch } = b;
  useEffect(() => {
    const { credential, id } = savedProject();
    if (credential) openProject(credential, id);
    patch({ selected: new URLSearchParams(window.location.search).get('browser') });
  }, [openProject, patch]);
}

/** Polls while there is a credential; returns the clock relative times use. */
function usePolling({ apiKey, live }: Base, { fetchBrowsers, fetchFleet, fetchPersonas }: Data) {
  const [now, setNow] = useState(Date.now);
  useEffect(
    () => (apiKey ? startPolling({ fetchBrowsers, fetchFleet, fetchPersonas }, live.hidden, setNow) : undefined),
    [apiKey, fetchBrowsers, fetchFleet, fetchPersonas, live],
  );
  return now;
}

/** Drops browsers that left the fleet from the selection. */
function usePruneSelection({ selected, checked }: ConsoleView, { browsers, loadError }: Data, patch: Base['patch']) {
  useEffect(
    () => pruneSelection(browsers, { selected, checked }, loadError, patch),
    [browsers, selected, checked, loadError, patch],
  );
}

/** Load-time and data-driven effects: restore, Slack return, polling, selection pruning. Returns the clock. */
function useConsoleEffects(b: Base, d: Data, openProject: (cred: string, id: string | null) => void, toast: Toast) {
  const { patch, view } = b;
  useRestore(openProject, b);
  const now = usePolling(b, d);
  useEffect(() => handleSlackReturn(patch, toast), [patch, toast]);
  usePruneSelection(view, d, patch);
  return now;
}

/** Whether a dialog or drawer is open; it handles Escape itself. */
const hasOverlay = (v: ConsoleView) => !!(v.showStart || v.showHelp || v.showSettings || v.stopIds || v.openPersona);

/** The console's shortcut list, rebuilt when the state its handlers read changes. */
function useShortcutList({ view, patch, filterRef, urlRef }: Base, requestStop: (ids: string[]) => void) {
  const { selected, checked } = view;
  const overlayOpen = hasOverlay(view);
  return useMemo(
    () => buildShortcuts({ selected, checked, overlayOpen, patch, filterRef, urlRef, requestStop }),
    [selected, checked, overlayOpen, patch, filterRef, urlRef, requestStop],
  );
}

/** The console's keyboard shortcuts, live unless there is no credential or the wizard is up. */
function useConsoleShortcuts(b: Base, requestStop: (ids: string[]) => void) {
  const shortcuts = useShortcutList(b, requestStop);
  useShortcuts(shortcuts, !!b.apiKey && !b.view.showOnboarding);
  return shortcuts;
}

/** What the page's buttons and panels call. */
function useActions(b: Base, d: Data, toast: Toast) {
  const { apiKey, patch, view } = b;
  const requestStop = useCallback((ids: string[]) => ids.length > 0 && patch({ stopIds: ids }), [patch]);
  const refresh = () => (d.fetchBrowsers(), d.fetchFleet());
  const doStop = () => stopBrowsers({ apiKey, view, patch, toast, refresh });
  const screenshotOf = useCallback((id: string) => takeScreenshot(apiKey, id, patch, toast), [apiKey, patch, toast]);
  const showBrowsersFor = useCallback((id: string) => patch(browsersOf(d.personas, id)), [d.personas, patch]);
  const onFilter = (n: Partial<FleetFilter>) => patch((v) => ({ filter: { ...v.filter, ...n } }));
  return { requestStop, refresh, doStop, screenshotOf, showBrowsersFor, onFilter };
}

/**
 * The fleet console's state. A thousand browsers in a table with their
 * health, one of them open on the right, and a keyboard to move between them.
 */
export function useConsole() {
  const toast = useToast();
  const base = useConsoleBase();
  const data = useData(base);
  const openProject = useOpenProject(base, data);
  const now = useConsoleEffects(base, data, openProject, toast);
  const actions = useActions(base, data, toast);
  const shortcuts = useConsoleShortcuts(base, actions.requestStop);
  return { ...base, ...data, ...actions, openProject, now, shortcuts };
}

/** Everything the console page renders from. */
export type Console = ReturnType<typeof useConsole>;
