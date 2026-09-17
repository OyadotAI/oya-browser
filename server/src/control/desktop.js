import { control, hash } from './service.js';
import { registry } from '../connection-registry.js';

export const desktopHolder = id => hash(`desktop:${id}`);
export function desktopState(id, state) {
  return { mode: state.mode, mine: state.holder === desktopHolder(id), expiresAt: state.expiresAt || null, taking: !!state.takeover && state.expiresAt > Date.now(), revision: state.revision || 0 };
}
export async function desktopControl(key, id, action, connected = () => true) {
  const holder = desktopHolder(id);
  if (!['get', 'request', 'acquire', 'renew', 'return'].includes(action)) throw new Error('Invalid control action');
  if (action === 'get') return desktopState(id, (await control().findSession(key, id)).control);
  if (action === 'return') {
    const state = (await control().findSession(key, id)).control;
    return desktopState(id, await control().takeover(key, id, state.mode === 'paused' && !state.holder ? 'resume' : 'return', holder));
  }
  if (action !== 'acquire') return desktopState(id, await control().takeover(key, id, action, holder));
  await control().takeover(key, id, 'request', holder);
  const deadline = Date.now() + 10000;
  do {
    if (!connected()) throw new Error('Browser disconnected during takeover');
    try {
      if (registry.get(id)?.pending) throw Object.assign(new Error('Current action is still running'), { code: 'commands_pending' });
      return desktopState(id, await control().takeover(key, id, 'acquire', holder));
    } catch (error) {
      if (error.code !== 'commands_pending') throw error;
    }
    await new Promise(resolve => setTimeout(resolve, 100));
  } while (Date.now() < deadline);
  throw new Error('Current action is still running. Automation remains paused; retry taking control or return to agent.');
}
