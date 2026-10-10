/** Workflow frame locators use native owner tokens rather than ambiguous frame URLs or stale wrappers. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { workflowScope } from '../../../../src/main/workflow/native-scope.ts';
/** Minimal native frame graph with duplicate URLs and independently owned tokens. */
function graph() {
  const parent: any = { detached: false, frames: [], _executeJavaScriptInOyaWorld: async () => 'second-token' };
  const first: any = {
    url: 'https://same.test',
    detached: false,
    parent,
    _oyaOwnerFrameToken: () => 'first-token',
    _executeJavaScriptInOyaWorld: async () => 'first',
  };
  const second: any = {
    url: 'https://same.test',
    detached: false,
    parent,
    _oyaOwnerFrameToken: () => 'second-token',
    _executeJavaScriptInOyaWorld: async () => 'second',
  };
  parent.frames = [first, second];
  const tab: any = { view: { webContents: { mainFrame: parent, isDestroyed: () => false } } };
  return { parent, first, second, tab, driver: {} as any };
}
it('selects the exact native child despite duplicate URLs', async () => {
  const g = graph();
  const scope = await workflowScope(g.driver, g.tab, ['iframe#second']);
  assert.equal(scope.frame, g.second);
  assert.equal(await scope.evaluate('read'), 'second');
});
it('revokes a resolved scope when its child owner token or parent changes', async () => {
  const g = graph();
  const scope = await workflowScope(g.driver, g.tab, ['iframe#second']);
  g.second._oyaOwnerFrameToken = () => 'replacement-token';
  await assert.rejects(scope.evaluate('read'), /frame changed/);
});
it('rejects top-document replacement before executing in an old native child', async () => {
  const g = graph();
  const scope = await workflowScope(g.driver, g.tab, ['iframe#second']);
  g.tab.view.webContents.mainFrame = {};
  await assert.rejects(scope.evaluate('read'), /document changed/);
});
it('refuses missing native owner lookup rather than guessing a frame index', async () => {
  const g = graph();
  g.parent._executeJavaScriptInOyaWorld = async () => undefined;
  await assert.rejects(workflowScope(g.driver, g.tab, ['iframe']), /capability unavailable/);
});
