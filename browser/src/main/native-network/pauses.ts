/** Native request continuations are owned capabilities, not arbitrary commands sent through a debugger. */
import { nativeResourceType } from './patterns.ts';
import { randomUUID } from 'node:crypto';
import type { WebContents } from 'electron';
import { leaseRequest, liveLease, type FrameLease } from './frame-lease.ts';
import { NETWORK_LIMITS } from './constants.ts';
import type { NetworkDependencies, RequestDetails, ResumeRequest, NetworkSink } from './types.ts';
/** One held native callback with a hard deadline. */
interface Pause {
  /** Immutable initiating native frame identity. */
  lease: FrameLease;
  /** Immutable initiating request. */
  details: RequestDetails;
  /** Native completion callback, consumed exactly once. */
  resume: ResumeRequest;
  /** Fail-closed deadline. */
  timer: ReturnType<typeof setTimeout>;
}
/** A context owns every pause, and a page can settle only its own pending request. */
export class NativePauses {
  /** Opaque external pause identities. */
  private readonly pending = new Map<string, Pause>();
  /** Current authorization and egress policy. */
  private readonly deps: NetworkDependencies;
  /** Capture browser-owned policy. */
  constructor(deps: NetworkDependencies) {
    this.deps = deps;
  }
  /** Publish only after installing the deadline and one-shot native continuation. */
  hold(details: RequestDetails, resume: ResumeRequest, emit: NetworkSink, networkId: string): void {
    if (this.pending.size >= NETWORK_LIMITS.paused) return resume({ cancel: true });
    const lease = leaseRequest(details);
    const id = randomUUID(),
      timer = setTimeout(() => this.finish(id, true), NETWORK_LIMITS.pauseMs);
    this.pending.set(id, { details, resume, timer, lease });
    this.announce(id, details, emit, networkId);
  }
  /** A disconnected/failed event sink cannot strand an unpublished native pause. */
  private announce(id: string, details: RequestDetails, emit: NetworkSink, networkId: string): void {
    try {
      emit('Fetch.requestPaused', pausedEvent(this.deps, details, id, networkId));
    } catch {
      this.finish(id, true);
    }
  }
  /** A continuation cannot change URL, method or headers, nor bypass current egress/human ownership. */
  resolve(contents: WebContents, id: string, cancel: boolean, reason?: string): void {
    const pause = this.pending.get(id);
    if (!pause || pause.details.webContents !== contents) throw Error('Unknown or foreign native paused request');
    if (!this.deps.allowed(contents) || !this.deps.allowedURL(pause.details.url) || !liveLease(contents, pause.lease)) {
      this.finish(id, true);
      throw Error('Native request continuation is no longer authorized');
    }
    this.finish(id, cancel, reason);
  }
  /** Consume before invoking native code so duplicate replies can never continue twice. */
  private finish(id: string, cancel: boolean, reason?: string): void {
    const pause = this.pending.get(id);
    if (!pause) return;
    this.pending.delete(id);
    clearTimeout(pause.timer);
    pause.resume({ cancel, ...(reason && reason !== 'BlockedByClient' ? { _oyaErrorReason: reason } : {}) });
  }
  /** Engine cancellation consumes its held callback immediately, not at the timeout deadline. */
  cancelRequest(nativeId: number): void {
    for (const [id, pause] of this.pending) if (pause.details.id === nativeId) this.finish(id, true);
  }
  /** A document commit revokes its old pending work; graph removal revokes detached descendants only. */
  changed(contents: WebContents, process?: number, routing?: number): void {
    for (const [id, pause] of this.pending) {
      if (pause.details.webContents !== contents) continue;
      const navigated = pause.lease.process === process && pause.lease.routing === routing;
      if (navigated || !liveLease(contents, pause.lease)) this.finish(id, true);
    }
  }
  /** Disabling/detaching fails closed rather than silently releasing unreviewed requests. */
  clear(contents?: WebContents): void {
    for (const [id, pause] of this.pending)
      if (!contents || pause.details.webContents === contents) this.finish(id, true);
  }
}
/** Only native request metadata is exposed; no fabricated stack traces or response-stage fields. */
function pausedEvent(deps: NetworkDependencies, d: RequestDetails, requestId: string, networkId: string) {
  return {
    requestId,
    networkId,
    frameId: deps.frameId(d.webContents!, d.frame!),
    resourceType: nativeResourceType(d.resourceType),
    request: { url: d.url, method: d.method, headers: d.requestHeaders || {} },
  };
}
