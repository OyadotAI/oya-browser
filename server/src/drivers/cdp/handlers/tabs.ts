/**
 * Tabs: listing, opening, switching and closing page targets.
 */
import type { CDPDriver } from '../driver.ts';
import type { Handler } from './types.ts';

/** The browser's page targets, marking the attached one. */
export const listTabs: Handler = async (driver) => {
  const { targetInfos = [] } = await driver.conn.send('Target.getTargets');
  const tabs = targetInfos
    .filter((t) => t.type === 'page')
    .map((t) => ({ id: t.targetId, url: t.url, title: t.title, active: t.targetId === driver.targetId }));
  return { ok: true, data: { tabs } };
};

/** Opens a tab and attaches to it. */
export const newTab: Handler = async (driver, params) => {
  const { targetId } = await driver.conn.send('Target.createTarget', { url: params.url || 'about:blank' });
  await driver.attach(targetId);
  return { ok: true, data: { id: targetId, tab_id: targetId } };
};

/** Attaches to another tab. */
export const switchTab: Handler = async (driver, params) => {
  if (!params.id) return { ok: false, error: 'tab id required' };
  await driver.attach(params.id);
  return { ok: true, data: { id: params.id } };
};

/** Closes a tab (the attached one by default), moving to another page if it was attached. */
export const closeTab: Handler = async (driver, params) => {
  const id = params.id || driver.targetId;
  await driver.conn.send('Target.closeTarget', { targetId: id });
  if (id === driver.targetId) await attachAnotherPage(driver, id);
  return { ok: true };
};

/** Attaches to any page but the one just closed. */
async function attachAnotherPage(driver: CDPDriver, closedId) {
  const { targetInfos = [] } = await driver.conn.send('Target.getTargets');
  // closeTarget can return before the tab leaves the list; never re-attach to it.
  const next = targetInfos.find((t) => t.type === 'page' && t.targetId !== closedId);
  if (next) await driver.attach(next.targetId);
}
