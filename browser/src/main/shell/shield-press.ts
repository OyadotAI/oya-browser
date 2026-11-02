/**
 * Notes a person pressing on the control shield in the tab health log. A page
 * whose clicks do nothing is either shielded or hung, and only this entry says
 * which: who held control, and what was mounted over the page. One entry per
 * control state and shown tab, however often the person clicks, so a dead page
 * never pushes the hang history out of the log.
 */
import type { BrowserView, WebContents } from 'electron';
import type { AppServices } from '../app/services.ts';

/** What the press reads: control, this window's tabs and views, and the log (absent in some tests). */
export type PressDeps = Pick<AppServices, 'control' | 'tabs' | 'shell'> & Partial<Pick<AppServices, 'tabHealthLog'>>;

/** What makes two presses the same episode: one control state over one shown tab. */
function episodeOf(deps: PressDeps): string {
  const { mode, mine, revision, expiresAt, busy } = deps.control.snapshot();
  return [mode, mine, revision, expiresAt, busy, deps.tabs.activeTabId].join('|');
}

/** Writes one press, with the window's views as they are mounted now. */
function logPress(deps: PressDeps, shield: BrowserView): void {
  const views = deps.shell.window?.getBrowserViews() ?? [];
  const tab = deps.tabs.activeTabId ?? null;
  deps.tabHealthLog?.shieldPress({ tabs: deps.tabs.list, views, shield, control: deps.control.snapshot(), tab });
}

/**
 * Logs the first mouse press on the shield (`contents`, its page) in each episode.
 * Electron's 'input-event' is used because the shield's other hook,
 * 'before-input-event', sees keys only.
 */
export function logShieldPresses(deps: PressDeps, contents: WebContents, shield: BrowserView): void {
  let logged = '';
  contents.on('input-event', (_event, input) => {
    if (input.type !== 'mouseDown' || !deps.tabHealthLog) return;
    const episode = episodeOf(deps);
    if (episode === logged) return;
    logged = episode;
    logPress(deps, shield);
  });
}
