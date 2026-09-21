/**
 * Links to this browser's live view for someone else, or for a new tab. The
 * token is scoped to one browser, and the fragment keeps it out of logs.
 */
import { api, errorMessage } from '@/lib/api-client';
import type { ShareToken } from '../fleet/open-stream';
import { withBusy, type PanelContext } from './context';

/** Mints a token scoped to this one browser and returns the full live link. */
export async function mintShare(ctx: PanelContext, control: boolean): Promise<string> {
  const id = encodeURIComponent(ctx.browserId);
  const r = await api<ShareToken>(`/control/sessions/${id}/share`, {
    key: ctx.apiKey,
    method: 'POST',
    body: { control },
  });
  return `${window.location.origin}/live/${id}#t=${encodeURIComponent(r.token)}`;
}

/** The toast after a link is copied; a control link warns that anyone holding it can act. */
const copiedMessage = (control: boolean) =>
  control ? 'Control link copied, expires in 1h, anyone with it can act' : 'View link copied, expires in 1h';

/** Copies the link; if the clipboard refuses, shows it in a toast instead. */
async function copyLink(ctx: PanelContext, url: string, control: boolean) {
  try {
    await navigator.clipboard.writeText(url);
    ctx.toast(copiedMessage(control), 'success');
  } catch {
    ctx.toast(url, 'info');
  }
}

/** Copy a shareable link to hand to someone else. */
export const share = (ctx: PanelContext, control: boolean) =>
  withBusy(ctx.setBusy, control ? 'share-control' : 'share-view', () =>
    mintShare(ctx, control)
      .then((url) => copyLink(ctx, url, control))
      .catch((e) => ctx.toast(errorMessage(e), 'error')),
  );

/** Sends an already-open tab to `url`, or opens one if the popup was blocked. */
function pointTab(tab: Window | null, url: string) {
  if (tab) tab.location.href = url;
  else window.open(url, '_blank');
}

/**
 * Open the live view in a new tab. A new tab does not inherit this tab's
 * credential (browsers force target=_blank to noopener), so carry a scoped
 * token in the link instead. Open the tab synchronously to keep the popup
 * within the click gesture, then point it once the token is minted.
 */
export function openStream(ctx: PanelContext) {
  const tab = window.open('', '_blank');
  const fail = (e: unknown) => (tab?.close(), ctx.toast(errorMessage(e), 'error'));
  return withBusy(ctx.setBusy, 'stream', () =>
    mintShare(ctx, true)
      .then((url) => pointTab(tab, url))
      .catch(fail),
  );
}
