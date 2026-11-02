/** Benign ownership probe: production popup adoption and passkey policy, synthetic native objects/accounts. */
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { mainCtx, FakeBrowserView, FakeWindow } from '../../../../tests/unit/support/main-ctx.cjs';
import { nativeProtectionFixture } from '../../../../tests/unit/support/native-protection.ts';
import { Protection } from '../../../../src/main/tabs/protection.ts';
import { TabWindows } from '../../../../src/main/tabs/tab-windows.ts';
import { adoptWindow } from '../../../../src/main/tabs/popup-tabs.ts';
import { WindowTabEvents } from '../../../../src/main/windows/tab-events.ts';
import { Passkeys } from '../../../../src/main/app/passkeys.ts';

test('production popup adoption cannot turn an agent-owned opener into human passkey consent', async () => {
  const ctx = mainCtx({ protection: Protection });
  const engine = nativeProtectionFixture();
  const state = { connected: true, mode: 'agent', localClients: 0, interactive: false };
  ctx.control.snapshot = () => state;
  ctx.persona.policyEpoch = Symbol();
  const agent = { id: 1, view: new FakeBrowserView() };
  const human = { id: 2, view: new FakeBrowserView() };
  agent.view.webContents.session = human.view.webContents.session = engine.session;
  ctx.tabs.list = [agent, human];
  ctx.tabs.activeTabId = human.id;
  ctx.tabs.find = (id) => ctx.tabs.list.find((tab) => tab.id === id);
  ctx.tabs.sendTabList = () => ctx.windows.tabEvents.changed();
  ctx.tabs.titleChanged = () => {};
  ctx.tabs.adoptWindow = (window) => adoptWindow(ctx.tabs, window, 3);
  ctx.windows = {
    owner: (id) => (ctx.tabs.find(id) ? ctx : undefined),
    ownerOfContents: (contents) => (ctx.tabs.list.some((tab) => tab.view.webContents === contents) ? ctx : undefined),
    tabEvents: new WindowTabEvents(),
  };
  const popup = new FakeWindow();
  popup.webContents.session = engine.session;
  popup.isFocused = () => true;
  popup.webContents.mainFrame = { parent: null, detached: false, isDestroyed: () => false };
  ctx.electron.webContents = {
    fromFrame: (frame) => (frame === popup.webContents.mainFrame ? popup.webContents : undefined),
  };
  const shield = [];
  ctx.shield.adoptPopup = (_window, own) => shield.push({ own });
  ctx.protection.configureSession(engine.session);
  ctx.agentTab.claim(agent.id);
  new TabWindows(ctx).wire(agent);
  agent.view.webContents.emit('did-create-window', popup, { url: 'https://fixture.test/sign-in' });
  const adopted = ctx.tabs.list.find((tab) => tab.window === popup);
  const prompts = [];
  ctx.electron.dialog.showMessageBox = async (_window, options) => {
    prompts.push(options.message);
    return { response: 1 };
  };
  const service = new Passkeys(ctx);
  const result = await service.choose({
    frame: popup.webContents.mainFrame,
    relyingPartyId: 'fixture.test',
    accounts: [{ name: 'Synthetic account', credentialId: 'fixture-only-id' }],
  });
  const evidence = {
    openerClaimed: !ctx.agentTab.humanMay(agent.id),
    popupOpenerId: adopted.openerId,
    popupHumanMay: ctx.agentTab.humanMay(adopted.id),
    shield,
    takeoverPrompts: ctx.shield.takeover.asked.length,
    passkeyPrompts: prompts.length,
    syntheticChoiceReturned: result === 'fixture-only-id',
    nativeFocusIsFake: true,
  };

  assert.equal(evidence.openerClaimed, true);
  assert.equal(evidence.popupHumanMay, true);
  assert.equal(evidence.takeoverPrompts, 0);
  assert.equal(evidence.syntheticChoiceReturned, false);
  assert.equal(evidence.passkeyPrompts, 0);
  ctx.agentTab.release();
});
