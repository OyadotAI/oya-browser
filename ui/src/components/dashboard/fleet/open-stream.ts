/**
 * Opens a browser's live stream in a new tab from the fleet table.
 */
import { api } from '@/lib/api-client';

/** What POST /control/sessions/:id/share answers. */
export interface ShareToken {
  /** A token scoped to one browser; it goes in the live link's fragment. */
  token: string;
}

/** Mints a control token scoped to one browser and returns its live-view path (token in the fragment). */
async function streamPath(apiKey: string, id: string): Promise<string> {
  const path = `/control/sessions/${encodeURIComponent(id)}/share`;
  const { token } = await api<ShareToken>(path, { key: apiKey, method: 'POST', body: { control: true } });
  return `/live/${encodeURIComponent(id)}#t=${encodeURIComponent(token)}`;
}

/**
 * A new tab does not inherit this tab's credential (browsers force target=_blank
 * to noopener), so carry a scoped token in the link. Open synchronously to stay
 * within the click gesture, then point the tab once the token is minted.
 */
export async function openStream(apiKey: string, id: string) {
  const tab = window.open('', '_blank');
  try {
    const url = await streamPath(apiKey, id);
    if (tab) tab.location.href = url;
    else window.open(url, '_blank');
  } catch {
    tab?.close();
  }
}
