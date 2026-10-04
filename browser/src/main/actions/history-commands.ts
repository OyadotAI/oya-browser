/**
 * Back, forward and reload in the tab a command targets, answered once the page
 * they lead to has loaded (or the wait runs out), with where the tab ended up.
 */
import type { WebContents } from 'electron';
import * as c from './constants.ts';
import type { PageHandler } from './page-commands.ts';
import type { PageDriver } from './driver.ts';
import type { CommandId } from './types.ts';
import type { PageView } from '../cdp/cdp.ts';

/** The tab history's ability checks. */
type CanGo = 'canGoBack' | 'canGoForward';
/** A move of the tab: back, forward or reload. */
type Move = (contents: WebContents) => void;
/** The tab history's moves. */
type Go = 'goBack' | 'goForward';

/** The events that say a history move has landed: a load finishing, or a move within the page. */
const LANDED = ['did-stop-loading', 'did-navigate-in-page'] as const;

/** Calls `fn` once, when the tab lands (LANDED) or LOAD_TIMEOUT_MS passes, then stops listening. */
function onceLanded(contents: WebContents, fn: () => void): void {
  const events: NodeJS.EventEmitter = contents;
  const done = (): void => {
    clearTimeout(timer);
    for (const event of LANDED) events.off(event, done);
    fn();
  };
  const timer = setTimeout(done, c.LOAD_TIMEOUT_MS);
  for (const event of LANDED) events.on(event, done);
}

/** Resolves once the tab finishes loading or moves within the page, or after LOAD_TIMEOUT_MS. */
const loaded = (contents: WebContents): Promise<void> => new Promise((resolve) => onceLanded(contents, resolve));

/** Runs `move` on the tab, waits for the page it leads to, and answers with that page. */
async function moveTab(driver: PageDriver, id: CommandId, view: PageView, move: Move): Promise<void> {
  const contents = view.webContents;
  const arrived = loaded(contents);
  move(contents);
  await arrived;
  await driver.deps.injectScripts(view);
  driver.deps.sendResult(id, true, { url: contents.getURL(), title: contents.getTitle() });
}

/** A step through the tab's history, refused when there is nowhere to go. */
const historyStep =
  (canGo: CanGo, go: Go, nowhere: string): PageHandler =>
  (driver, id, _params, view) => {
    const history = view.webContents.navigationHistory;
    if (!history[canGo]()) return driver.deps.sendResult(id, false, null, nowhere);
    return moveTab(driver, id, view, () => history[go]());
  };

/** The handler for each history command. */
export const HISTORY_COMMANDS: Readonly<Record<string, PageHandler>> = {
  back: historyStep('canGoBack', 'goBack', 'There is no page to go back to.'),
  forward: historyStep('canGoForward', 'goForward', 'There is no page to go forward to.'),
  reload: (driver, id, _params, view) => moveTab(driver, id, view, (contents) => contents.reload()),
};
