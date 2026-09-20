/**
 * What the proxies dialog's buttons do. Each runs through `run`, so it marks
 * itself busy and toasts a failure.
 */
import { checkSummary, type ProxyRow } from './model';
import { addProxy, checkProxies, deleteProxy } from './persona-api';
import type { ProxiesState } from './use-proxies';

/** Adds the drafted proxy, clears its label, URL and country, and reloads. */
export const add = (s: ProxiesState) =>
  s.run('add', async () => {
    const p = await addProxy(s.props.apiKey, s.draft);
    s.toast(`Added ${p.label}`, 'success');
    s.set({ label: '', url: '', geo: '' });
    await s.load();
    s.props.onChanged();
  });

/** Checks every proxy and says how many failed. */
export const check = (s: ProxiesState) =>
  s.run('check', async () => {
    s.toast(...checkSummary(await checkProxies(s.props.apiKey)));
    await s.load();
  });

/** The first click asks to confirm; the second removes the proxy and reloads. */
export function remove(s: ProxiesState, p: ProxyRow) {
  if (s.confirming !== p.id) return s.setConfirming(p.id);
  return s.run(p.id, () => removeNow(s, p)).then(() => s.setConfirming(null));
}

/** Removes the proxy, says so, and reloads. */
async function removeNow(s: ProxiesState, p: ProxyRow) {
  await deleteProxy(s.props.apiKey, p.id);
  s.toast(`Removed ${p.label}`, 'success');
  await s.load();
  s.props.onChanged();
}
