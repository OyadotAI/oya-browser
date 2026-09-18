/**
 * The record the registry keeps for each connected browser: what the caller
 * said about it, plus live state and counters that start empty.
 */

/** What a caller says about a browser when it registers it. */
export type BrowserSpec = {
  /** Inbound client socket (Oya); absent for an outbound driver. */
  ws?: any;
  /** Key the browser belongs to. */
  apiKey: string;
  /** Display name. */
  name: string;
  /** Outbound driver (CDP) this server dialled. */
  driver?: any;
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
};

/** How the browser is reached: its socket or driver, and what kind of client it is. */
const reach = ({ ws, cdp = false, driver = null, clientType = 'oya' }: BrowserSpec) => ({
  ws,
  cdp,
  driver,
  clientType,
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
export const newRecord = (spec: BrowserSpec) => ({ ...reach(spec), ...origin(spec), ...live(), ...history() });
