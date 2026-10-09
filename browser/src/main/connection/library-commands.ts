/** Agent access to the same local library and closed-tab stack the human uses. */
import type { TabHandler } from './tab-commands.ts';
import { isWebAddress, NOT_A_WEB_ADDRESS } from '../tabs/navigation.ts';
import { reopenClosed } from '../tabs/tab-order.ts';
import { whenProtected } from '../tabs/load.ts';

/** Reopening must not bypass the remote open_tab restriction on local files. */
function reopenWebTab(tabs: Parameters<typeof reopenClosed>[0]): number | undefined {
  const entry = tabs.closed.entries.at(-1);
  if (entry && !isWebAddress(entry.url)) throw new Error(NOT_A_WEB_ADDRESS);
  return reopenClosed(tabs);
}
/** Reopen through the tab manager, so protection and normal tab ordering still apply. */
const reopen: TabHandler = async (runner, id) => {
  const { tabs, actions } = runner.deps;
  const tabId = reopenWebTab(runner.deps.windows?.current.tabs ?? tabs);
  if (tabId === undefined) return runner.sendResult(id, true, { reopened: false });
  const tab = tabs.find(tabId);
  await whenProtected(tab);
  await actions.waitForTabReady(tab);
  runner.sendResult(id, true, { reopened: true, tab_id: tabId, url: tab?.url });
};
/** Local operations run behind the normal socket admission/control gate, never a separate backdoor. */
export const LIBRARY_COMMANDS: Record<string, TabHandler> = {
  search_history: (runner, id, args) => runner.sendResult(id, true, runner.deps.library.search('history', args)),
  list_bookmarks: (runner, id, args) => runner.sendResult(id, true, runner.deps.library.search('bookmarks', args)),
  add_bookmark: (runner, id, args) =>
    runner.sendResult(id, true, { bookmark: runner.deps.library.addBookmark(args?.url, args?.title) }),
  remove_bookmark: (runner, id, args) =>
    runner.sendResult(id, true, { removed: runner.deps.library.removeBookmark(args?.url) }),
  clear_history: (runner, id, args) => {
    if (args?.confirm !== true)
      throw new Error('Clearing history requires confirm: true after an explicit user request');
    runner.deps.library.clearHistory();
    runner.sendResult(id, true, { cleared: true });
  },
  list_closed_tabs: (runner, id) =>
    runner.sendResult(id, true, {
      tabs: [...runner.deps.tabs.closed.entries].reverse().filter((entry) => isWebAddress(entry.url)),
    }),
  reopen_closed_tab: reopen,
};
