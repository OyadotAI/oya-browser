/** A single authorized document owns a bounded, cancellable native recorder lifecycle. */
import { randomUUID } from 'node:crypto';
import { withinTime } from '../../shared/within-time.ts';
import { NATIVE_RECORDING_COMMAND_MS } from './constants.ts';
import { evaluateFrame } from './frames.ts';
import type { NativeRecordingDocument } from './recording-document.ts';
import { recorderCommandScript, startRecorderScript, STOP_RECORDER } from './recorder-scripts.ts';
/** Never retarget a replacement document or let an old stop command stop a newer recording. */
export class NativeDocumentRecorder {
  /** Immutable authorization-time attribution. */
  private readonly document: NativeRecordingDocument;
  /** The inbox epoch accepted by the isolated sink. */
  private readonly epoch: string;
  /** The analyzer source, with recording disabled until explicitly armed. */
  private readonly analyzer: string;
  /** Unique ownership prevents stale lifecycle commands from changing another recorder. */
  private readonly owner = randomUUID();
  /** Repeated starts share one installation and readiness result. */
  private ready?: Promise<void>;
  /** Stopping is terminal even before the page becomes ready. */
  private closed = false;
  /** Concurrent stops share final draining rather than taking the buffer twice. */
  private stopping?: Promise<unknown>;
  /** Construction does not execute scripts or subscribe to any protocol. */
  constructor(document: NativeRecordingDocument, epoch: string, analyzer: string) {
    this.document = document;
    this.epoch = epoch;
    this.analyzer = analyzer;
  }
  /** Resolve only when the actual recorder is armed, not merely when a DOM-ready listener exists. */
  start(): Promise<void> {
    if (this.closed) return Promise.reject(new Error('Document recorder is stopped'));
    this.ready ??= this.arm();
    return this.ready;
  }
  /** A failed installation gets a bounded best-effort cleanup without hiding its original failure. */
  private async arm(): Promise<void> {
    try {
      const script = startRecorderScript(this.document.documentId, this.owner, this.epoch, this.analyzer);
      if ((await this.execute(script)) !== true) throw new Error('Document recorder was cancelled before readiness');
    } catch (error) {
      await this.stop().catch(() => {});
      throw error;
    }
  }
  /** Every execution is document guarded and bounded; no stale frame or debugging fallback exists. */
  private execute(script: string): Promise<unknown> {
    return withinTime(
      evaluateFrame(this.document.frame, script),
      NATIVE_RECORDING_COMMAND_MS,
      'Native recorder did not answer',
    );
  }
  /** Drain buffered steps; the recording domain deduplicates their ids against streamed batches. */
  async drain(final = false): Promise<unknown> {
    if (!this.ready || this.closed) throw new Error('Document recorder is not running');
    await this.ready;
    if (this.closed) throw new Error('Document recorder is stopped');
    return this.command(`return window.__acRecordDrain(${!!final});`);
  }
  /** Discard unfinished edits and secret names without stopping this authorized document. */
  async clear(): Promise<void> {
    if (!this.ready || this.closed) throw new Error('Document recorder is not running');
    await this.ready;
    if (this.closed) throw new Error('Document recorder is stopped');
    await this.command('window.__acRecordClear();');
  }
  /** Scope commands to this lifecycle even if another recorder has since started in the same document. */
  private command(script: string): Promise<unknown> {
    return this.execute(recorderCommandScript(this.document.documentId, this.owner, script));
  }
  /** Cancel pending readiness immediately; never wait for DOMContentLoaded before stopping. */
  stop(): Promise<unknown> {
    this.closed = true;
    this.stopping ??= this.ready ? this.command(STOP_RECORDER) : Promise.resolve(undefined);
    return this.stopping;
  }
}
