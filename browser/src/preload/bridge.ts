/**
 * Builds window.oyaBrowser from the contract in shared/ipc.ts: one method per
 * call channel and one subscription per event channel. Electron's IPC comes in
 * as an argument, so the bridge is tested without Electron.
 */
import { CALL_CHANNELS, EVENT_CHANNELS, type OyaBrowser } from '../shared/ipc.ts';

/** The part of ipcRenderer the bridge uses. */
export interface RendererIpc {
  /** Calls a channel's handler in the main process. */
  invoke(channel: string, ...args: unknown[]): Promise<unknown>;
  /** Listens on a channel. */
  on(channel: string, listener: (event: unknown, payload: unknown) => void): unknown;
  /** Stops listening on a channel. */
  removeListener(channel: string, listener: (event: unknown, payload: unknown) => void): unknown;
}

/** The call's name in the bridge. */
type CallName = keyof typeof CALL_CHANNELS;

/** Arguments only the preload can supply, from the page's media queries. */
function filledIn(matches: (query: string) => boolean): Partial<Record<CallName, () => unknown[]>> {
  // The panel's slide follows the person's reduced-motion setting.
  return { toggleDevPanel: () => [matches('(prefers-reduced-motion: reduce)')] };
}

/** One method per call: invokes its channel with the page's arguments, or the ones filled in here. */
function calls(ipc: RendererIpc, matches: (query: string) => boolean) {
  const fills = filledIn(matches);
  return Object.entries(CALL_CHANNELS).map(([name, channel]) => {
    const fill = fills[name as CallName];
    return [name, (...args: unknown[]) => ipc.invoke(channel, ...(fill ? fill() : args))] as const;
  });
}

/** A channel's listeners: one IPC listener per channel, fanned out, so many ViewModels can share an event. */
class Fanout {
  /** The page's listeners on this channel. */
  private readonly listeners = new Set<(payload: unknown) => void>();
  /** Whether the IPC listener is on. */
  private attached = false;
  /** Electron's IPC. */
  private readonly ipc: RendererIpc;
  /** The channel. */
  private readonly channel: string;

  /** For `channel` over `ipc`. */
  constructor(ipc: RendererIpc, channel: string) {
    this.ipc = ipc;
    this.channel = channel;
  }

  /** Hands every listener the payload alone. */
  private readonly relay = (_event: unknown, payload: unknown): void => {
    for (const listener of [...this.listeners]) listener(payload);
  };

  /** Adds a listener (attaching to IPC on the first), and returns how to remove it (detaching after the last). */
  add(listener: (payload: unknown) => void): () => void {
    this.listeners.add(listener);
    if (!this.attached) this.ipc.on(this.channel, this.relay);
    this.attached = true;
    return () => this.remove(listener);
  }

  /** Removes a listener; IPC is let go once none are left. */
  private remove(listener: (payload: unknown) => void): void {
    this.listeners.delete(listener);
    if (this.listeners.size || !this.attached) return;
    this.ipc.removeListener(this.channel, this.relay);
    this.attached = false;
  }
}

/** One subscription per event: hands the listener the payload alone, and returns how to stop listening. */
function events(ipc: RendererIpc) {
  return Object.entries(EVENT_CHANNELS).map(([name, channel]) => {
    const fanout = new Fanout(ipc, channel);
    return [name, (listener: (payload: unknown) => void) => fanout.add(listener)] as const;
  });
}

/** window.oyaBrowser over `ipc`; `matches` answers media queries. */
export function buildBridge(ipc: RendererIpc, matches: (query: string) => boolean): OyaBrowser {
  return Object.fromEntries([...calls(ipc, matches), ...events(ipc)]) as unknown as OyaBrowser;
}
