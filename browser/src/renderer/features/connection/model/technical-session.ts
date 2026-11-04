/** Share a cancellable authenticated workspace wait across direct login and pending provider setup. */
import type { OyaBrowser } from '../../../core/bridge.ts';
import { RendererConstants as C } from '../../../core/constants.ts';
import type { ConnectionStatus } from '../../../../shared/ipc.ts';

/** Only main-owned connection operations participate in setup. */
export type TechnicalSessionBridge = Pick<
  OyaBrowser,
  'beginOnboarding' | 'onboardingStatus' | 'startDesktopSignIn' | 'saveConfig' | 'onWsStatus'
>;
/** Keep signatures readable while the public contract retains its descriptive export. */
type Bridge = TechnicalSessionBridge;
/** Accessible progress stays with the initiating form. */
type Progress = (status: string) => void;
/** A saved workspace reconnects directly; a fresh one authenticates without an Oya-key form. */
export async function connectWorkspace(bridge: Bridge, signal: AbortSignal, progress: Progress): Promise<void> {
  const status = await prepare(bridge, signal);
  if (status.connected) return;
  await authorize(bridge, !!status.signedIn, progress);
  signal.throwIfAborted();
  progress('Connecting browser…');
  return workspaceReady(bridge, signal);
}
/** Hold setup before an asynchronous authentication may connect the desktop. */
async function prepare(bridge: Bridge, signal: AbortSignal) {
  await bridge.beginOnboarding();
  signal.throwIfAborted();
  const status = await bridge.onboardingStatus();
  signal.throwIfAborted();
  return status;
}
/** Reconnection retains main's sealed key; login uses the correlated native service. */
async function authorize(bridge: Bridge, signedIn: boolean, progress: Progress) {
  if (signedIn) return bridge.saveConfig({});
  progress('Waiting for Oya sign-in…');
  const answer = await bridge.startDesktopSignIn();
  if (answer.error) throw Error(answer.error);
}
/** The listener and snapshot belong to one cancellable wait. */
class WorkspaceWait {
  /** Main-owned socket bridge. */ private readonly bridge: Bridge;
  /** Flow navigation owns this wait. */ private readonly signal: AbortSignal;
  /** Remove socket subscription on every exit. */ private off = () => {};
  /** Deadline prevents a paired-but-disconnected browser hanging setup. */ private timer?: ReturnType<
    typeof setTimeout
  >;
  /** A raced snapshot/event settles only once. */ private done = false;
  /** Completion handles are set before subscribing to any event. */ private resolve = () => {};
  /** Rejections stay inside the flow's error handling. */ private reject: (error: Error) => void = () => {};
  /** Capture one connection lifetime. */
  constructor(bridge: Bridge, signal: AbortSignal) {
    this.bridge = bridge;
    this.signal = signal;
  }
  /** Install lifecycle handlers only after completion callbacks exist. */
  run(): Promise<void> {
    return new Promise((resolve, reject) => {
      this.resolve = resolve;
      this.reject = reject;
      this.watch();
    });
  }
  /** A snapshot covers authentication that finished before this subscription. */
  private watch(): void {
    this.deadline();
    this.off = this.bridge.onWsStatus((status) => this.update(status));
    if (this.done) return this.off();
    this.signal.addEventListener('abort', this.abort, { once: true });
    this.snapshot();
    if (this.signal.aborted) this.abort();
  }
  /** A paired browser must authenticate within the existing connection budget. */
  private deadline(): void {
    this.timer = setTimeout(
      () => this.finish(Error('Signed in. Browser connection failed. Reconnect to try again.')),
      C.CONNECT_TIMEOUT_MS,
    );
  }
  /** Snapshot failure leaves the bounded socket wait available. */
  private snapshot(): void {
    void this.bridge.onboardingStatus().then(
      (status) => this.update({ connected: !!status.connected }),
      () => {},
    );
  }
  /** Unrelated disconnected events cannot finish an authenticated wait. */
  private update(status: ConnectionStatus): void {
    if (status.connected || status.failure) this.finish(status.failure ? Error(status.failure) : undefined);
  }
  /** Bound arrow removes the exact abort subscription. */
  private readonly abort = () => this.finish(Error('Connection cancelled.'));
  /** Settle once and release every local resource. */
  private finish(error?: Error): void {
    if (this.done) return;
    this.done = true;
    clearTimeout(this.timer);
    this.off();
    this.signal.removeEventListener('abort', this.abort);
    if (error) this.reject(error);
    else this.resolve();
  }
}
/** Expose the wait's behavior without its implementation state. */
export const workspaceReady = (bridge: Bridge, signal: AbortSignal): Promise<void> =>
  new WorkspaceWait(bridge, signal).run();
