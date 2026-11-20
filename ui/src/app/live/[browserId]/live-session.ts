/**
 * The live page's work outside React: resolving the credential, streaming
 * frames with an fps and age meter, and taking or handing back control.
 */
import { api, errorMessage } from '@/lib/api-client';
import { consoleCredential } from '@/lib/api';
import { MS_PER_SECOND } from '@/lib/constants';
import { subscribeFrames } from '@/lib/live-stream';

/** Where the live view's state goes. */
export interface LiveSink {
  /** The browser's name, for the heading. */
  name: (name: string) => void;
  /** The latest frame, or null while there is none. */
  frame: (frame: string | null) => void;
  /** Frames received in the last second. */
  fps: (fps: number) => void;
  /** Milliseconds since the last frame, or null before the first. */
  age: (age: number | null) => void;
  /** The error line, empty for none. */
  error: (error: string) => void;
  /** The browser's current control mode (agent, human, paused). */
  mode?: (mode: string) => void;
}

/** Session-storage key prefix for a shared link's scoped token, scoped per browser. */
const SHARE_PREFIX = 'oya_live_share:';

/** Saves a share token in tab storage so refreshing keeps the live view authenticated. */
function rememberShare(browserId: string, token: string) {
  try {
    sessionStorage.setItem(SHARE_PREFIX + browserId, token);
  } catch {}
}

/** Reads a previously saved share token for `browserId`, or ''. */
function savedShare(browserId: string): string {
  try {
    return sessionStorage.getItem(SHARE_PREFIX + browserId) || '';
  } catch {
    return '';
  }
}

/**
 * A shared link carries its scoped token in the URL fragment (#t=…), which
 * never reaches the server or a Referer header. It is persisted in this tab's
 * sessionStorage keyed by browserId so reloads stay authenticated, and stripped
 * from the URL once read. Falls back to the console credential when absent.
 */
export function takeSharedCredential(browserId = ''): string {
  const shared = new URLSearchParams(window.location.hash.slice(1)).get('t');
  if (shared && browserId) rememberShare(browserId, shared);
  if (shared) history.replaceState(null, '', window.location.pathname + window.location.search);
  return shared || (browserId ? savedShare(browserId) : '') || consoleCredential();
}

/** Counts frames so the page can show fps and frame age once a second. */
class FrameMeter {
  /** Frames since the last report. */
  private frames = 0;
  /** When the last frame arrived; 0 before the first. */
  private lastFrame = 0;

  /** A frame arrived. */
  hit() {
    this.frames++;
    this.lastFrame = Date.now();
  }

  /** Reports fps and age, then starts counting again. */
  report(sink: LiveSink) {
    sink.fps(this.frames);
    this.frames = 0;
    sink.age(this.lastFrame ? Date.now() - this.lastFrame : null);
  }
}

/** A frame arrived: show it, count it, and clear any error. */
function received(sink: LiveSink, meter: FrameMeter, frame: string) {
  sink.frame(frame);
  meter.hit();
  sink.error('');
}

/** The stream dropped; it reconnects by itself. */
function lost(sink: LiveSink) {
  sink.frame(null);
  sink.error('The live stream disconnected. Reconnecting…');
}

/** Starts the stream for a browser that was found; returns its stop. */
function stream(browserId: string, apiKey: string, sink: LiveSink, meter: FrameMeter) {
  return subscribeFrames(
    browserId,
    apiKey,
    (frame) => received(sink, meter, frame),
    () => lost(sink),
  );
}

/** The part of a browser record the page needs. */
interface BrowserName {
  /** The browser's display name. */
  name: string;
  /** The browser's current control mode when included by `/browsers/:id`. */
  controlMode?: string;
}

/** Control session details returned by `/control/sessions/:id`. */
interface ControlSessionInfo {
  /** Mode field at the top level. */
  mode?: string;
  /** Nested control object containing mode. */
  control?: {
    /** Who drives the browser now. */
    mode?: string;
  };
}

/** Reads the browser's current control mode from `/control/sessions/:id`, or undefined. */
async function fetchControlMode(browserId: string, apiKey: string): Promise<string | undefined> {
  const path = `/control/sessions/${encodeURIComponent(browserId)}`;
  const info = await api<ControlSessionInfo>(path, { key: apiKey }).catch(() => null);
  return info?.control?.mode ?? info?.mode;
}

/** Context shared while connecting a live view stream. */
interface LiveTarget {
  /** Target browser id. */
  browserId: string;
  /** Active credential. */
  apiKey: string;
  /** Where state updates go. */
  sink: LiveSink;
  /** Frame rate meter. */
  meter: FrameMeter;
  /** Whether this view attempt is still active. */
  alive: () => boolean;
}

/** Syncs initial control mode from the browser record or control session. */
function syncInitialMode(t: LiveTarget, known: string | undefined) {
  if (known) return t.sink.mode?.(known);
  void fetchControlMode(t.browserId, t.apiKey).then((mode) => t.alive() && mode && t.sink.mode?.(mode));
}

/** A control or input answer. */
export interface CommandAnswer {
  /** Whether the browser carried it out. */
  ok: boolean;
  /** Why not, when it did not. */
  error?: string;
}

/** One control step: the request and the button text. */
export interface ControlStep {
  /** What to ask the control endpoint for. */
  action: string;
  /** The button text while in this mode. */
  label: string;
}

/** Attaches the stream and syncs control mode after `/browsers/:id` responds. */
function attachBrowser(browser: BrowserName, t: LiveTarget) {
  t.sink.name(browser.name);
  syncInitialMode(t, browser.controlMode);
  return stream(t.browserId, t.apiKey, t.sink, t.meter);
}

/** Looks the browser up, syncs its control mode, and streams its frames; returns the stop for all of it. */
export function openLiveView(browserId: string, apiKey: string, sink: LiveSink): () => void {
  let cancelled = false;
  let stopStream: (() => void) | undefined;
  const target: LiveTarget = { browserId, apiKey, sink, meter: new FrameMeter(), alive: () => !cancelled };
  api<BrowserName>(`/browsers/${encodeURIComponent(browserId)}`, { key: apiKey })
    .then((b) => target.alive() && (stopStream = attachBrowser(b, target)))
    .catch((err) => target.alive() && sink.error(errorMessage(err)));
  const timer = setInterval(() => target.meter.report(sink), MS_PER_SECOND);
  return () => void ((cancelled = true), stopStream?.(), clearInterval(timer));
}

/** What the control button does and says in each mode; any other mode resumes the agent. */
const CONTROL: Record<string, ControlStep> = {
  agent: { action: 'acquire', label: 'Take control' },
  human: { action: 'release', label: 'Release control' },
};
/** The control button for a paused or unknown mode. */
const RESUME: ControlStep = { action: 'resume', label: 'Resume agent' };

/** The control step for `mode`. */
export function controlFor(mode: string) {
  return Object.hasOwn(CONTROL, mode) ? CONTROL[mode] : RESUME;
}

/** The mode the control endpoint answers with. */
interface ModeAnswer {
  /** Who drives the browser now: agent, human or paused. */
  mode: string;
}

/** Asks for the next control step after `mode`; returns the new mode. */
export async function changeControl(browserId: string, apiKey: string, mode: string): Promise<string> {
  const body = { action: controlFor(mode).action };
  const path = `/control/sessions/${encodeURIComponent(browserId)}/control`;
  const c = await api<ModeAnswer>(path, { key: apiKey, method: 'POST', body });
  return c.mode;
}

/** Sends one input command to the browser; a refused command throws. */
export async function sendInput(browserId: string, apiKey: string, action: string, params: Record<string, unknown>) {
  const response = await api<CommandAnswer>(`/control/sessions/${encodeURIComponent(browserId)}/input`, {
    key: apiKey,
    method: 'POST',
    body: { action, params },
  });
  if (!response.ok) throw new Error(response.error || 'Command failed');
  return response;
}
