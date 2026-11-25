/** Library tools exercise the desktop dispatcher and real result envelope. */
import { it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { CommandRunner } from '../../../../src/main/connection/commands.ts';
import { BrowsingLibrary } from '../../../../src/main/library/index.ts';
import { TabManager } from '../../../../src/main/tabs/tabs.ts';
import { mainCtx } from '../../support/main-ctx.cjs';
let ctx;
beforeEach(() => {
  mock.timers.enable({ apis: ['setTimeout'] });
  ctx = mainCtx({ tabs: TabManager, library: BrowsingLibrary, commands: CommandRunner });
  ctx.actions = { waitForTabReady: async () => {} };
});
afterEach(() => mock.timers.reset());
/** No page automation is needed for library reads. */
async function run(action, params = {}) {
  await ctx.commands.handleCommand({ id: 'library', action, params });
  return ctx.socket.ofType('cmd_result').at(-1);
}
it('reads and mutates the library even without an active page', async () => {
  assert.equal((await run('add_bookmark', { url: 'https://a.test/', title: 'A' })).ok, true);
  assert.equal((await run('list_bookmarks', { query: 'A' })).data.entries[0].title, 'A');
  assert.equal((await run('remove_bookmark', { url: 'https://a.test/' })).data.removed, true);
  assert.equal((await run('add_bookmark', { url: 'file:///private' })).ok, false);
  assert.equal((await run('search_history', { limit: -1 })).ok, false);
});
it('requires boolean confirmation and preserves bookmarks when clearing history', async () => {
  ctx.library.visit('https://a.test/', 'A');
  ctx.library.addBookmark('https://a.test/', 'A');
  for (const confirm of [undefined, false, 'true', 1])
    assert.equal((await run('clear_history', { confirm })).ok, false);
  assert.equal((await run('search_history')).data.total, 1);
  assert.equal((await run('clear_history', { confirm: true })).data.cleared, true);
  assert.equal((await run('search_history')).data.total, 0);
  assert.equal((await run('list_bookmarks')).data.total, 1);
});
it('lists closed web tabs, skips non-web entries, and assigns the reopened tab to the calling lane without stealing focus', async () => {
  await ctx.commands.handleCommand({
    id: 'seed',
    lane: 'mcp-1',
    action: 'open_tab',
    params: { url: 'https://seed.test/' },
  });
  const humanTab = ctx.tabs.createTab('https://human.test/', true);
  ctx.control.state = { mode: 'agent', mine: false, interactive: false, connected: true, localClients: 0 };
  (ctx.agentTab as any).claims.set('mcp-1', 1);
  ctx.tabs.activateTab(humanTab);
  ctx.tabs.closed.push('https://a.test/', 0);
  ctx.tabs.closed.push('file:///private', 1);
  assert.equal((await run('list_closed_tabs')).data.tabs.length, 1);
  await ctx.commands.handleCommand({ id: 'reopen-1', lane: 'mcp-1', action: 'reopen_closed_tab' });
  const reopened = ctx.socket.ofType('cmd_result').at(-1);
  assert.equal(reopened.data.reopened, true);
  assert.equal(ctx.tabs.find(reopened.data.tab_id).url, 'https://a.test/');
  assert.equal(ctx.tabs.activeTabId, humanTab, 'background agent does not steal the active human tab');
  assert.deepEqual([...ctx.tabs.closed.entries.map((e) => e.url)], ['file:///private']);
  await ctx.commands.handleCommand({
    id: 'close-1',
    lane: 'mcp-1',
    action: 'close_tab',
    params: { tab_id: reopened.data.tab_id },
  });
  assert.equal(ctx.socket.ofType('cmd_result').at(-1).ok, true, 'lane owns the tab it reopened');
  ctx.tabs.closed.entries.pop(); // drop the re-closed https://a.test/
  assert.equal((await run('reopen_closed_tab')).data.reopened, false, 'file:///private is never reopened by an agent');
  assert.equal(ctx.tabs.closed.size, 1);
});
it('reports protection failure instead of claiming successful reopening', async () => {
  ctx.tabs.closed.push('https://a.test/', 0);
  ctx.tabs.createTab = () => 7;
  ctx.tabs.find = () => ({ id: 7, protection: 'failed' });
  assert.equal((await run('reopen_closed_tab')).ok, false);
});
