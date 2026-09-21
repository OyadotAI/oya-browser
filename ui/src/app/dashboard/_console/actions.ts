/**
 * What the console does on request, as plain functions: stop browsers, take
 * a screenshot, show a persona's browsers, and react to a Slack install.
 */
import { api, errorMessage } from '@/lib/api-client';
import type { useToast } from '@/components/dashboard/toast';
import type { BrowserRow, Persona } from '@/components/dashboard/types';
import type { Patch } from '@/lib/hooks/use-patch-state';
import { NO_FILTER, type ConsoleView } from './view';

/** Raises a toast. */
export type Toast = ReturnType<typeof useToast>;

/** One browser's stop result. */
interface StopResult {
  /** The browser. */
  id: string;
  /** Whether it stopped. */
  ok: boolean;
  /** Whether its sandbox was removed; null when it had none. */
  sandboxRemoved: boolean | null;
  /** Why it did not stop. */
  error?: string;
}

/** The answer of POST /browsers/stop. */
interface StopAnswer {
  /** How many stopped. */
  stopped: number;
  /** Per-browser results. */
  results: StopResult[];
}

/** What a stop needs. */
export interface StopContext {
  /** The credential. */
  apiKey: string;
  /** The current view. */
  view: ConsoleView;
  /** Changes the view. */
  patch: Patch<ConsoleView>;
  /** Raises a toast. */
  toast: Toast;
  /** Refreshes the table and strip. */
  refresh: () => void;
}

/** The toast for a stop: how many stopped, and any sandboxes left behind. */
export function stopToast(r: StopAnswer): [string, 'error' | 'success'] {
  const failed = r.results.filter((x) => x.sandboxRemoved === false).length;
  const note = failed ? `, ${failed} sandbox${failed > 1 ? 'es' : ''} could not be removed` : '';
  return [`Stopped ${r.stopped}${note}`, failed ? 'error' : 'success'];
}

/** Stops the browsers awaiting confirmation. */
export async function stopBrowsers(c: StopContext) {
  const ids = c.view.stopIds;
  if (!ids) return;
  c.patch({ stopping: true });
  await stopAndReport(c, ids)
    .catch((err) => c.toast(errorMessage(err), 'error'))
    .finally(() => c.patch({ stopping: false, stopIds: null }));
}

/** Sends the stop, reports it, and clears the selection of what stopped. */
async function stopAndReport(c: StopContext, ids: string[]) {
  const r = await api<StopAnswer>('/browsers/stop', { key: c.apiKey, method: 'POST', body: { ids } });
  c.toast(...stopToast(r));
  if (c.view.selected && ids.includes(c.view.selected)) c.patch({ selected: null });
  c.patch({ checked: new Set() });
  c.refresh();
}

/** A screenshot command's payload. */
interface ShotData {
  /** The image as a data URL. */
  screenshot?: string;
}

/** The answer of a screenshot command. */
interface ShotAnswer {
  /** Whether the command ran. */
  ok: boolean;
  /** The screenshot, when it did. */
  data?: ShotData;
  /** Why not. */
  error?: string;
}

/** Takes a screenshot of a browser and shows it. */
export async function takeScreenshot(apiKey: string, id: string, patch: Patch<ConsoleView>, toast: Toast) {
  try {
    const body = { action: 'screenshot' };
    const r = await api<ShotAnswer>(`/browsers/${id}/command`, { key: apiKey, method: 'POST', body });
    if (r.data?.screenshot) patch({ shot: { id, src: r.data.screenshot } });
    else toast(r.error || 'No screenshot', 'error');
  } catch (err) {
    toast(errorMessage(err), 'error');
  }
}

/** The view that lists one persona's browsers. */
export function browsersOf(personas: Persona[], personaId: string): Partial<ConsoleView> {
  const p = personas.find((x) => x.id === personaId);
  return { filter: { ...NO_FILTER, persona: p?.name || personaId }, openPersona: null, tab: 'browsers' };
}

/**
 * Coming back from the Slack install. Land the person on the channel picker
 * rather than an unchanged dashboard, and drop the parameter so a refresh is quiet.
 */
export function handleSlackReturn(patch: Patch<ConsoleView>, toast: Toast) {
  const outcome = new URLSearchParams(window.location.search).get('slack');
  if (!outcome) return;
  history.replaceState(null, '', window.location.pathname);
  if (outcome === 'pick-channel') {
    patch({ settingsSection: 'alerts', showSettings: true });
    toast('Slack connected, choose a channel', 'success');
  } else if (outcome === 'connected') toast('Slack connected', 'success');
  else toast(`Slack install failed: ${outcome.replace(/_/g, ' ')}`, 'error');
}

/** The selected and ticked browsers. */
type Selection = Pick<ConsoleView, 'selected' | 'checked'>;

/** A browser that leaves the fleet leaves the selection too. */
export function pruneSelection(
  browsers: BrowserRow[],
  view: Selection,
  loadError: string | null,
  patch: Patch<ConsoleView>,
) {
  const { selected } = view;
  if (selected && !loadError && !browsers.some((b) => b.id === selected)) patch({ selected: null });
  if (view.checked.size) pruneChecked(browsers, view.checked, patch);
}

/** Unticks browsers that are gone. */
function pruneChecked(browsers: BrowserRow[], checked: Set<string>, patch: Patch<ConsoleView>) {
  const alive = new Set(browsers.map((b) => b.id));
  const next = new Set([...checked].filter((id) => alive.has(id)));
  if (next.size !== checked.size) patch({ checked: next });
}
