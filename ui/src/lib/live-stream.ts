/**
 * The live view's frame stream: a server-sent event stream that reconnects
 * on its own. Each EventSource connection uses a fresh, one-use ticket, never
 * a project credential in its URL.
 */
import { apiUrl, authHeaders } from './api';
import { LIVE_RETRY_MS } from './constants';

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
  private readonly onError: () => void;

  /** Streams `id`'s frames with `key`, reporting frames and losses to the callbacks. */
  constructor(id: string, key: string, onFrame: (frame: string) => void, onError: () => void) {
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
    } catch {
      if (!this.closed) this.failed();
    }
  }

  /** Reports a failure and reconnects after a pause. */
  private failed() {
    this.onError();
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
    if (!res.ok) throw new Error('Cannot obtain a live-view ticket');
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
export function subscribeFrames(id: string, key: string, onFrame: (frame: string) => void, onError: () => void) {
  const stream = new FrameStream(id, key, onFrame, onError);
  void stream.connect();
  return () => stream.close();
}
