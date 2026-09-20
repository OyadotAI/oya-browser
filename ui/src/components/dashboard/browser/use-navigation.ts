/**
 * The URL bar: what is typed, going there, and reloading.
 */
import { useState } from 'react';
import type { BrowserDetail } from '../types';
import { withBusy, type PanelContext } from './context';

/** An input command and its params. */
type Command = [action: string, params?: Record<string, unknown>];

/**
 * The Oya client has no reload over the socket; navigating to the same
 * URL is the same thing from the page's point of view.
 */
export function reloadCommand(d: BrowserDetail | null): Command | null {
  if (d?.clientType === 'cdp') return ['reload'];
  return d?.currentUrl ? ['navigate', { url: d.currentUrl }] : null;
}

/** Navigates to `target`, then polls so the new page shows. A blank target does nothing. */
async function navigate(ctx: PanelContext, target: string, setUrl: (u: string | null) => void) {
  const t = target.trim();
  if (!t) return;
  await withBusy(ctx.setBusy, 'navigate', async () => {
    setUrl(null);
    ctx.onInput(`navigate ${t}`);
    await ctx.send('navigate', { url: t });
  });
  ctx.refresh();
}

/** Reloads the page the browser is on. */
function reload(ctx: PanelContext) {
  return withBusy(ctx.setBusy, 'reload', async () => {
    ctx.onInput('reload');
    const cmd = reloadCommand(ctx.detail);
    if (cmd) await ctx.send(...cmd);
  });
}

/** `url` is what is typed (null shows the current page); `go` navigates there. */
export function useNavigation(ctx: PanelContext) {
  const [url, setUrl] = useState<string | null>(null);
  const go = (target = url ?? ctx.detail?.currentUrl ?? '') => navigate(ctx, target, setUrl);
  return { url, setUrl, go, reload: () => reload(ctx) };
}
