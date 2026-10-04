/**
 * Right-click: everything you can do to one browser, without hunting for a button.
 */
import { Camera, Copy, ExternalLink, PanelRightOpen, Plug, Square } from 'lucide-react';
import type { MenuItem } from '@/components/ui/context-menu';
import type { BrowserRow } from '../types';
import { api, errorMessage } from '@/lib/api-client';
import { openStream } from './open-stream';

/** What the menu's items call back into. */
export interface RowMenuActions {
  /** Key that authorises the stream token and the one-use CDP ticket; it never goes in a URL. */
  apiKey: string;
  /** Opens the browser in the side panel. */
  onSelect: (id: string) => void;
  /** Opens the connect dialog for the browser. */
  onConnect: (id: string) => void;
  /** Takes a screenshot of the browser. */
  onScreenshot: (id: string) => void;
  /** Stops the given browsers. */
  onStop: (ids: string[]) => void;
  /** Says how an action went: a copy made, a stream that would not open. */
  notify: (message: string, kind: 'success' | 'error') => void;
}

/** What GET /browsers/:id answers, as far as attaching goes. */
interface AttachDetail {
  /** A CDP URL carrying a one-use ticket that expires in 60 seconds; absent when the browser offers no CDP. */
  cdpUrl?: string;
}

/** This page's http origin. */
function httpOrigin() {
  return typeof window !== 'undefined' ? `${window.location.protocol}//${window.location.host}` : '';
}

/** A fresh ticketed CDP URL from the server, so the copied URL never carries the key. */
async function freshCdpUrl(apiKey: string, id: string): Promise<string> {
  const { cdpUrl } = await api<AttachDetail>(`/browsers/${encodeURIComponent(id)}`, { key: apiKey });
  if (!cdpUrl) throw new Error('this browser offers no CDP URL');
  return cdpUrl;
}

/** Copies `text` (awaited, so a fetch that fails is reported too) and says so; a clipboard that refuses is said too. */
async function copyText(text: string | Promise<string>, what: string, notify: RowMenuActions['notify']) {
  try {
    await navigator.clipboard.writeText(await text);
    notify(`${what} copied`, 'success');
  } catch (err) {
    notify(`Could not copy: ${errorMessage(err)}`, 'error');
  }
}

/** The copy items: id, MCP URL, and a one-use CDP attach URL (only CDP browsers can take it). */
function copyItems(r: BrowserRow, { apiKey, notify }: RowMenuActions): MenuItem[] {
  const http = httpOrigin();
  const cdp = r.clientType === 'cdp';
  return [
    { label: 'Copy browser id', icon: <Copy />, separator: true, onSelect: () => copyText(r.id, 'Browser id', notify) },
    { label: 'Copy MCP URL', icon: <Copy />, onSelect: () => copyText(`${http}/mcp/${r.id}`, 'MCP URL', notify) },
    {
      label: cdp ? 'Copy CDP attach URL (one use, 60 s)' : 'Copy CDP attach URL, not a CDP browser',
      icon: <Copy />,
      disabled: !cdp,
      onSelect: () => copyText(freshCdpUrl(apiKey, r.id), 'CDP attach URL', notify),
    },
  ];
}

/** Every action for one row, in menu order. */
export function rowMenuItems(r: BrowserRow, a: RowMenuActions): MenuItem[] {
  const stopLabel = r.provider === 'oya-cloud' ? 'Stop, destroys the sandbox' : 'Stop';
  return [
    { label: 'Open', icon: <PanelRightOpen />, shortcut: '↵', onSelect: () => a.onSelect(r.id) },
    { label: 'Connect… (code, Playwright, MCP)', icon: <Plug />, onSelect: () => a.onConnect(r.id) },
    { label: 'Screenshot', icon: <Camera />, shortcut: 'S', onSelect: () => a.onScreenshot(r.id) },
    {
      label: 'Open live stream in a tab',
      icon: <ExternalLink />,
      onSelect: () => void openStream(a.apiKey, r.id, (m) => a.notify(m, 'error')),
    },
    ...copyItems(r, a),
    {
      label: stopLabel,
      icon: <Square />,
      shortcut: 'X',
      danger: true,
      separator: true,
      onSelect: () => a.onStop([r.id]),
    },
  ];
}
