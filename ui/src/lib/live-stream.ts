/**
 * The live view's frame stream: a server-sent event stream that reconnects
 * on its own. Each EventSource connection uses a fresh, one-use ticket, never
 * a project credential in its URL.
 */
import { apiUrl, authHeaders } from './api';
import { ApiError } from './api-client';
import { Status } from './http-status';
import { LIVE_RETRY_MS } from './constants';

/** Ticket refusals that require a new credential or a new browser. */
const TERMINAL = new Set<number>([Status.UNAUTHORIZED, Status.FORBIDDEN, Status.NOT_FOUND, Status.GONE]);

/** A live page can stop retries after a terminal refusal. Other subscribers recover as before. */
type StreamError = (status?: number) => boolean | void;

/** Delivers one captured browser frame. */
type FrameHandler = (frame: string) => void;

/** One subscription: fetches a ticket, opens the stream, and retries after a failure until closed. */
class FrameStream {
  /** Set once the subscriber has gone; nothing reconnects after that. */
  private closed = false;
  /** The open stream, once there is one. */
  private source: EventSource | undefined;
  /** The pending reconnect, if any. */
  private retry: ReturnType<typeof setTimeout> | undefined;
  /** Cancels an in-flight ticket request on close. */
  private readonly abort = new AbortController();
  /** The browser whose frames these are. */
  private readonly id: string;
  /** The credential that buys the ticket. */
  private readonly key: string;
  /** Receives each frame (a data URL). */
  private readonly onFrame: (frame: string) => void;
  /** Told whenever the stream is lost or cannot be opened. */
  private readonly onError: StreamError;

  /** Streams the browser with this credential, reporting pictures and connection losses. */
  constructor(id: string, key: string, onFrame: FrameHandler, onError: StreamError) {
    this.id = id;
    this.key = key;
    this.onFrame = onFrame;
    this.onError = onError;
  }

  /** Gets a ticket and opens the stream; a failure is reported and retried. */
  async connect() {
    try {
      const ticket = await this.ticket();
      if (this.closed) return;
      this.open(ticket);
    } catch (err) {
      if (!this.closed) this.failed(err);
    }
  }

  /** Reports a failure and reconnects after a pause. */
  private failed(err: unknown) {
    const status = err instanceof ApiError && TERMINAL.has(err.status) ? err.status : undefined;
    if (this.onError(status) === false) return;
    this.scheduleRetry();
  }

  /** Stops for good: cancels the ticket request, closes the stream and any pending retry. */
  close() {
    this.closed = true;
    this.abort.abort();
    this.source?.close();
    clearTimeout(this.retry);
  }

  /** A one-use ticket for the stream URL. */
  private async ticket(): Promise<string> {
    const url = apiUrl(`/control/sessions/${encodeURIComponent(this.id)}/ticket`);
    const init = { method: 'POST', headers: authHeaders(this.key), body: '{}', signal: this.abort.signal };
    const res = await fetch(url, init);
    if (!res.ok) throw new ApiError('Cannot obtain a live-view ticket', res.status, null);
    const { ticket } = await res.json();
    return ticket;
  }

  /** Opens the event stream with `ticket`. */
  private open(ticket: string) {
    this.source = new EventSource(apiUrl(`/live/${encodeURIComponent(this.id)}?ticket=${encodeURIComponent(ticket)}`));
    this.source.onmessage = (e) => this.onFrame(e.data);
    this.source.onerror = () => this.lost();
  }

  /** The stream dropped: close it, report it, and reconnect unless closed. */
  private lost() {
    this.source?.close();
    this.onError();
    if (!this.closed) this.scheduleRetry();
  }

  /** Reconnects after a pause. */
  private scheduleRetry() {
    this.retry = setTimeout(() => void this.connect(), LIVE_RETRY_MS);
  }
}

/** Streams a browser's frames until the returned function is called. */
export function subscribeFrames(id: string, key: string, onFrame: FrameHandler, onError: StreamError) {
  const stream = new FrameStream(id, key, onFrame, onError);
  void stream.connect();
  return () => stream.close();
}
