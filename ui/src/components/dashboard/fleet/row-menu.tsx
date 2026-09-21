/**
 * Right-click: everything you can do to one browser, without hunting for a button.
 */
import { Camera, Copy, ExternalLink, PanelRightOpen, Plug, Square } from 'lucide-react';
import type { MenuItem } from '@/components/ui/context-menu';
import type { BrowserRow } from '../types';
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
}

/** This page's http origin, and the same origin as a WebSocket URL. */
function origins() {
  const http = typeof window !== 'undefined' ? `${window.location.protocol}//${window.location.host}` : '';
  return { http, ws: http.replace(/^http/, 'ws') };
}

/** The copy items: id, MCP URL, and the CDP attach URL (only CDP browsers can take it). */
function copyItems(r: BrowserRow, apiKey: string): MenuItem[] {
  const { http, ws } = origins();
  const copy = (t: string) => navigator.clipboard.writeText(t);
  const cdp = r.clientType === 'cdp';
  return [
    { label: 'Copy browser id', icon: <Copy />, separator: true, onSelect: () => copy(r.id) },
    { label: 'Copy MCP URL', icon: <Copy />, onSelect: () => copy(`${http}/mcp/${r.id}`) },
    {
      label: cdp ? 'Copy CDP attach URL (with key)' : 'Copy CDP attach URL, not a CDP browser',
      icon: <Copy />,
      disabled: !cdp,
      onSelect: () => copy(`${ws}/connect?token=${encodeURIComponent(apiKey)}&browser=${r.id}`),
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
    { label: 'Open live stream in a tab', icon: <ExternalLink />, onSelect: () => void openStream(a.apiKey, r.id) },
    ...copyItems(r, a.apiKey),
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
