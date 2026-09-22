/**
 * The record the registry keeps for each connected browser: what the caller
 * said about it, plus live state and counters that start empty.
 */

import type { CDPDriver } from '../../../drivers/cdp.ts';
import { driverFor, type BrowserDriver } from '../driver/index.ts';

/** What a caller says about a browser when it registers it. */
export type BrowserSpec = {
  /** Inbound client socket (Oya); absent for a browser this server dialled. */
  ws?: any;
  /** Key the browser belongs to. */
  apiKey: string;
  /** Display name. */
  name: string;
  /** The CDP engine this server dialled the browser with; absent for an Oya browser. */
  engine?: CDPDriver | null;
  /** 'oya' or 'cdp'. */
  clientType?: string;
  /** Vendor or source that supplied the browser. */
  provider?: string | null;
  /** Hands a hosted session back to its vendor on removal. */
  release?: (() => unknown) | null;
  /** Identity the browser is running as. */
  persona?: any;
  /** An inbound Oya browser that also offers CDP through the relay. */
  cdp?: boolean;
  /** The actions an inbound Oya browser said it does, already checked; absent from an older app. */
  actions?: readonly string[] | null;
};

/** How the browser is reached: its driver, picked here once, its socket, and what kind of client it is. */
const reach = (spec: BrowserSpec, browserId: string) => ({
  ws: spec.ws,
  cdp: spec.cdp ?? false,
  driver: driverFor(spec, browserId) as BrowserDriver,
  clientType: spec.clientType ?? 'oya',
});

/** Where it came from and whom it belongs to. */
const origin = ({ provider = null, release = null, persona = null, apiKey, name }: BrowserSpec) => ({
  provider,
  release,
  persona,
  apiKey: apiKey || '',
  name: name || 'Unknown Browser',
});

/** Connection times, the page it is on and its live-view state. */
const live = () => ({
  connectedAt: new Date(),
  lastSeen: new Date(),
  currentUrl: '',
  lastFrame: null,
  lastFrameAt: null,
  streamViewers: new Set(),
});

/** What this browser has been doing. Bounded; newest first. */
const history = () => ({
  activity: [],
  commands: 0,
  errors: 0,
  pending: 0,
  lastCommandAt: null,
  lastError: null,
});

/** A fresh record for a browser that just connected. */
export const newRecord = (spec: BrowserSpec, browserId: string) => ({
  ...reach(spec, browserId),
  ...origin(spec),
  ...live(),
  ...history(),
});
