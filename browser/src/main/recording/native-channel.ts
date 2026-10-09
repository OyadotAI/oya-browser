/** One native recorder per authorized document, surviving navigation and cross-process frame changes. */
import type { WebContents, WebFrameMain } from 'electron';
import {
  NativeRecordingInbox,
  NativeDocumentRecorder,
  recordingDocumentIsCurrent,
  type NativeRecordingDocument,
  type NativeRecordingMessage,
} from '../native/index.ts';
import { withinTime } from '../../shared/within-time.ts';
import { NATIVE_RECORDING_SETTLE_MS, NATIVE_CAPTURE_ISSUE } from './constants.ts';
import { RecordingFrames } from './native-frames.ts';
import { nativeRecordingOutput } from './native-output.ts';
import type { PageOutput } from './types.ts';
/** Immutable attribution and the lifecycle owner for one current frame document. */
interface ArmedDocument {
  /** Browser-authorized identity and path. */
  document: NativeRecordingDocument;
  /** Exact isolated recorder lifecycle, never a debugging connection. */
  recorder: NativeDocumentRecorder;
}
/** Native admission and document lifetimes replace bindings, Runtime domains and child debugging sessions. */
export class NativeRecordingChannel {
  /** Only this page's native graph and IPC are eligible. */
  private readonly page: WebContents;
  /** Analyzer source is trusted browser code, installed in the agent isolated world. */
  private readonly analyzer: string;
  /** Existing recording domain retains normalization, deduplication and control cutoffs. */
  private readonly receive: (output: PageOutput) => void;
  /** Retains final-unload message attribution until the whole recording stops. */
  private readonly inbox: NativeRecordingInbox;
  /** Per-frame native readiness/removal subscription. */
  private readonly frames: RecordingFrames;
  /** Only the latest document controller for each live frame is kept. */
  private readonly armed = new Map<WebFrameMain, ArmedDocument>();
  /** Coalesce concurrent arming for one exact frame without dropping a later ready event. */
  private readonly pending = new Map<WebFrameMain, Promise<void>>();
  /** A new document arrived while its prior arm operation was in flight. */
  private readonly dirty = new Set<WebFrameMain>();
  /** The epoch supplied only to isolated recorders. */
  private epoch = '';
  /** Starts and stops are idempotent and cannot restart a disposed channel. */
  private started?: Promise<void>;
  /** Stop shares one bounded final-drain result. */
  private stopping?: Promise<void>;
  /** Late native operations are fenced before any listener is removed. */
  private closed = false;
  /** Construction never attaches a debugger or begins recording implicitly. */
  constructor(page: WebContents, analyzer: string, receive: (output: PageOutput) => void) {
    this.page = page;
    this.analyzer = analyzer;
    this.receive = receive;
    this.inbox = new NativeRecordingInbox(page, (message) => this.message(message));
    this.frames = this.observer();
  }
  /** Frame callbacks retain this exact channel, not a globally selected tab. */
  private observer(): RecordingFrames {
    return new RecordingFrames(this.page, {
      ready: (frame) => this.changed(frame),
      removed: (frame) => this.armed.delete(frame),
      failed: () => this.issue(),
    });
  }

  /** Initial readiness means the real current documents have installed their recorders. */
  start(): Promise<void> {
    if (this.closed) return Promise.reject(Error('Native recording channel is stopped'));
    this.started ??= this.begin();
    return this.started;
  }
  /** Register IPC before arming; unwind subscriptions even when initial graph inspection fails. */
  private async begin(): Promise<void> {
    this.epoch = this.inbox.start();
    try {
      await Promise.all(this.frames.start().map((frame) => this.initial(frame, this.page.mainFrame)));
    } catch (error) {
      await this.stop().catch(() => {});
      throw error;
    }
  }
  /** A missing root recorder refuses Start; an unsupported child becomes an explicit workflow warning. */
  private async initial(frame: WebFrameMain, main: WebFrameMain): Promise<void> {
    try {
      await this.schedule(frame);
    } catch (error) {
      if (frame === main) throw error;
      this.issue();
    }
  }
  /** Native navigation readiness is event-driven; background failures remain visible in the workflow. */
  private changed(frame: WebFrameMain): void {
    if (!this.closed) void this.schedule(frame).catch(() => this.issue());
  }
  /** Concurrent events join one arm operation and arrange another identity check afterward. */
  private schedule(frame: WebFrameMain): Promise<void> {
    if (this.closed || frame.detached) return Promise.resolve();
    const existing = this.pending.get(frame);
    if (existing) this.dirty.add(frame);
    if (existing) return existing;
    const task = this.arm(frame).finally(() => this.finished(frame));
    this.pending.set(frame, task);
    return task;
  }
  /** Recheck a readiness event that arrived during the previous asynchronous identity lookup. */
  private finished(frame: WebFrameMain): void {
    this.pending.delete(frame);
    if (this.dirty.delete(frame)) this.changed(frame);
  }
  /** Identity is captured before installation; replacement documents never inherit old recorder ownership. */
  private async arm(frame: WebFrameMain): Promise<void> {
    const document = await this.authorize(frame);
    if (this.closed || frame.detached || this.armed.get(frame)?.document.documentId === document.documentId) return;
    const recorder = new NativeDocumentRecorder(document, this.epoch, this.analyzer);
    const entry = { document, recorder };
    this.armed.set(frame, entry);
    await recorder.start();
    if (this.closed) await recorder.stop();
  }
  /** Bound topology/identity reads even if a document never becomes script-ready. */
  private authorize(frame: WebFrameMain): Promise<NativeRecordingDocument> {
    return withinTime(
      this.inbox.authorize(frame),
      NATIVE_RECORDING_SETTLE_MS,
      'Native recording authorization timed out',
    );
  }
  /** Streamed final unload messages keep their old document's URL and owner chain. */
  private message(message: NativeRecordingMessage): void {
    try {
      this.deliver(JSON.parse(message.payload), message.document);
    } catch {
      this.issue();
    }
  }
  /** Preserve immutable frame attribution while the existing domain deduplicates streamed and drained steps. */
  private deliver(value: unknown, document: NativeRecordingDocument): void {
    if (value !== undefined) this.receive(nativeRecordingOutput(value, document));
  }
  /** Never turn an unsupported frame or malformed batch into a silently successful recording. */
  private issue(): void {
    if (!this.closed) this.receive({ steps: [{ action: 'unsupported_frame', captureIssue: NATIVE_CAPTURE_ISSUE }] });
  }
  /** Pending initialization is observed before reading the latest available controllers. */
  async drain(final = false): Promise<void> {
    if (this.closed) throw Error('Native recording channel is stopped');
    await withinTime(this.settled(), NATIVE_RECORDING_SETTLE_MS, 'Native recording did not settle');
    for (const entry of [...this.armed.values()]) await this.visit(entry, false, final);
  }
  /** Discard buffered and unfinished edits in every current document without switching recorder worlds. */
  async clear(): Promise<void> {
    if (this.closed) throw Error('Native recording channel is stopped');
    await withinTime(this.settled(), NATIVE_RECORDING_SETTLE_MS, 'Native recording did not settle');
    for (const entry of [...this.armed.values()])
      if (await recordingDocumentIsCurrent(this.page, entry.document)) await entry.recorder.clear();
  }
  /** Readiness received during an in-flight authorization must finish before draining its replacement. */
  private async settled(): Promise<void> {
    while (this.pending.size && !this.closed) await Promise.all([...this.pending.values()]);
  }
  /** Detached contexts have already flushed through IPC; live failures are not disguised as success. */
  private async visit(entry: ArmedDocument, stop: boolean, final = false): Promise<void> {
    if (!(await recordingDocumentIsCurrent(this.page, entry.document))) return;
    const value = stop ? await entry.recorder.stop() : await entry.recorder.drain(final);
    this.deliver(value, entry.document);
  }
  /** Stop new work immediately, then preserve final typing before closing IPC admission. */
  stop(): Promise<void> {
    this.closed = true;
    this.frames.stop();
    this.stopping ??= this.finish();
    return this.stopping;
  }
  /** Stop every live controller even if one refuses; no failed stop leaves sibling recorders running. */
  private async finish(): Promise<void> {
    const stopping = [...this.armed.values()].map((entry) => this.visit(entry, true));
    const results = await Promise.allSettled(stopping);
    await Promise.allSettled([...this.pending.values()]);
    this.inbox.stop();
    this.armed.clear();
    this.dirty.clear();
    const failed = results.find((result) => result.status === 'rejected');
    if (failed?.status === 'rejected' && !this.page.isDestroyed()) throw failed.reason;
  }
}
