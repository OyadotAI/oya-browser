/**
 * Right-click: everything you can do to one browser, without hunting for a button.
 */
import { Camera, Copy, ExternalLink, PanelRightOpen, Plug, Square } from 'lucide-react';
import type { MenuItem } from '@/components/ui/context-menu';
import type { BrowserRow } from '../types';
import { errorMessage } from '@/lib/api-client';
import { openStream } from './open-stream';

/** What the menu's items call back into. */
export interface RowMenuActions {
  /** Key that authorises the stream token and goes in the CDP attach URL. */
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

/** This page's http origin, and the same origin as a WebSocket URL. */
function origins() {
  const http = typeof window !== 'undefined' ? `${window.location.protocol}//${window.location.host}` : '';
  return { http, ws: http.replace(/^http/, 'ws') };
}

/** Copies `text` and says so; a clipboard that refuses (no permission, no focus) is said too. */
async function copyText(text: string, what: string, notify: RowMenuActions['notify']) {
  try {
    await navigator.clipboard.writeText(text);
    notify(`${what} copied`, 'success');
  } catch (err) {
    notify(`Could not copy: ${errorMessage(err)}`, 'error');
  }
}

/** The copy items: id, MCP URL, and the CDP attach URL (only CDP browsers can take it). */
function copyItems(r: BrowserRow, { apiKey, notify }: RowMenuActions): MenuItem[] {
  const { http, ws } = origins();
  const cdp = r.clientType === 'cdp';
  const attach = `${ws}/connect?token=${encodeURIComponent(apiKey)}&browser=${r.id}`;
  return [
    { label: 'Copy browser id', icon: <Copy />, separator: true, onSelect: () => copyText(r.id, 'Browser id', notify) },
    { label: 'Copy MCP URL', icon: <Copy />, onSelect: () => copyText(`${http}/mcp/${r.id}`, 'MCP URL', notify) },
    {
      label: cdp ? 'Copy CDP attach URL (with key)' : 'Copy CDP attach URL, not a CDP browser',
      icon: <Copy />,
      disabled: !cdp,
      onSelect: () => copyText(attach, 'CDP attach URL', notify),
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
