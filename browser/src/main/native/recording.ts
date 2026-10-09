/** Own native recording IPC admission without trusting payload-supplied frame identities. */
import { randomUUID } from 'node:crypto';
import type { IpcMainEvent, WebContents, WebFrameMain } from 'electron';
import { captureDocument, type NativeRecordingDocument } from './recording-document.ts';
import { NATIVE_RECORDING } from '../../shared/native-recording.ts';
/** Browser-supplied source attribution accompanies each still-unparsed batch. */
export interface NativeRecordingMessage {
  /** Actual sending frame, never a frame id supplied by the renderer payload. */
  frame: WebFrameMain;
  /** Bounded serialized recorder output; the recording domain validates its contents. */
  payload: string;
  /** Immutable authorization-time attribution survives final unload batches. */
  document: NativeRecordingDocument;
}
/** Explicit ownership and fresh epochs reject late batches from a previous recording. */
export class NativeRecordingInbox {
  /** The one page whose native IPC events this inbox accepts. */
  private readonly page: WebContents;
  /** The owning recorder consumes admitted batches synchronously. */
  private readonly receive: (message: NativeRecordingMessage) => void;
  /** Only a started inbox may deliver a batch. */
  private active = false;
  /** A new recording cannot inherit the previous recording's queued messages. */
  private epoch = '';
  /** Keep old documents through final flushes; stop releases the entire recording scope. */
  private readonly documents = new Map<string, NativeRecordingDocument>();
  /** Stable listener identity permits exact teardown. */
  private readonly listener = (event: IpcMainEvent, ...args: unknown[]): void => {
    const [channel, epoch, payload, documentId] = args;
    if (!this.active || channel !== NATIVE_RECORDING.CHANNEL || epoch !== this.epoch || event.sender !== this.page)
      return;
    if (typeof payload !== 'string' || payload.length > NATIVE_RECORDING.MAX_PAYLOAD_CHARS) return;
    const frame = event.senderFrame;
    const document = typeof documentId === 'string' ? this.documents.get(documentId) : undefined;
    if (frame && document?.frame === frame) this.receive({ frame, payload, document });
  };
  /** Closing a page releases subscriptions, not just its pending data. */
  private readonly destroyed = (): void => this.stop();
  /** No debugger, remote endpoint, filesystem or renderer-supplied target enters this inbox. */
  constructor(page: WebContents, receive: (message: NativeRecordingMessage) => void) {
    this.page = page;
    this.receive = receive;
  }
  /** Register before arming a recorder, returning the epoch its isolated sink must use. */
  start(): string {
    if (this.page.isDestroyed()) throw new Error('View is destroyed');
    if (this.active) return this.epoch;
    this.epoch = randomUUID();
    this.active = true;
    this.page.on('ipc-message', this.listener);
    this.page.on('destroyed', this.destroyed);
    return this.epoch;
  }
  /** Authorize a live document before installing its recorder; restarting invalidates in-flight work. */
  async authorize(frame: WebFrameMain): Promise<NativeRecordingDocument> {
    if (!this.active) throw new Error('Recording inbox is stopped');
    const epoch = this.epoch;
    const document = await captureDocument(this.page, frame);
    if (!this.active || this.epoch !== epoch) throw new Error('Recording changed while arming');
    return this.remember(document);
  }
  /** Re-arming is idempotent; a collision cannot replace another frame's authorization. */
  private remember(document: NativeRecordingDocument): NativeRecordingDocument {
    const previous = this.documents.get(document.documentId);
    if (previous && previous.frame !== document.frame) throw new Error('Recording document identity collision');
    if (previous) return previous;
    if (this.documents.size >= NATIVE_RECORDING.MAX_DOCUMENTS)
      throw new Error('Native recording document limit exceeded');
    this.documents.set(document.documentId, document);
    return document;
  }
  /** Stop admission before removing listeners, so stale callbacks cannot deliver. */
  stop(): void {
    this.active = false;
    this.documents.clear();
    this.page.off('ipc-message', this.listener);
    this.page.off('destroyed', this.destroyed);
  }
}
