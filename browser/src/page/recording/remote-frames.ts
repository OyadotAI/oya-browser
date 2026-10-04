/**
 * Cross-site iframes in a recording. Each runs in its own process, a separate
 * target with its own CDP session, so the page's channel cannot arm it. Here
 * each gets a channel of its own on its session, and its steps come out with
 * the iframe's place in the page in front of their own frame path.
 *
 * Optional: a transport that cannot talk to child sessions passes no `frames`,
 * and those iframes stay unrecorded as before.
 */

import { frameSelector, type Send } from './frames.ts';
import { RECORDING } from '../constants.ts';

/** Subscribes to one CDP event; returns the unsubscribe. */
export type On = (event: string, handler: (params: unknown) => void) => () => unknown;

/** A transport to one CDP session: commands out, events in. */
export interface Port {
  /** Sends one command. */
  send: Send;
  /** Subscribes to one event. */
  on: On;
}

/** One recorded step, in the page's shape; only its frame path is read here. */
export interface RecordedStep {
  /** The frame selectors from the page down to the step's frame. */
  frames?: string[];
  /** Anything else the step carries. */
  [field: string]: unknown;
}

/** What a recorder delivers: steps, and anything else it reports (secret names). */
export interface RecorderOutput {
  /** The recorded steps. */
  steps?: RecordedStep[];
  /** Anything else. */
  [field: string]: unknown;
}

/** Hands on what a recorder delivered. */
export type Receive = (out: RecorderOutput) => unknown;

/** One iframe session: its CDP session id and the iframe's frame id. */
export interface FrameSession {
  /** The CDP session id. */
  sessionId: string;
  /** The iframe's frame (target) id. */
  frameId: string;
}

/** Lists, watches and talks to a page's cross-site iframe sessions (src/main/recording/frame-sessions.ts). */
export interface FrameSessions {
  /** The iframe sessions attached now. */
  list(): FrameSession[];
  /** Hears each iframe that attaches; returns the unwatch. */
  watch(attached: (sessionId: string, frameId: string) => void): () => unknown;
  /** A transport to one iframe's session. */
  port(sessionId: string): Port;
}

/** What RemoteFrames needs of the page's channel. */
export interface ParentChannel {
  /** Sends a command to the page's session. */
  send: Send;
  /** Hands steps on. */
  receive: Receive;
  /** The selectors from the top frame down to `frameId`. */
  framePath(frameId: string): Promise<string[]>;
}

/** What RemoteFrames needs of an iframe's channel. */
export interface ChildChannel {
  /** Arms its recorder. */
  start(): Promise<unknown>;
  /** Disarms it. */
  stop(): Promise<unknown>;
  /** Collects what it buffered. */
  drain(final?: boolean): Promise<unknown>;
  /** Discards what it holds. */
  clear(): Promise<unknown>;
}

/** An iframe's port, with where its channel delivers. */
export interface ChildPort extends Port {
  /** Hands on what the iframe's recorder delivered. */
  receive: Receive;
}

/** Builds an iframe's channel on its port, delivering through `receive`. */
export type MakeChannel = (port: ChildPort) => ChildChannel;

/** A step's frame path, or none and why. */
export interface Located {
  /** The frame selectors from the page down. */
  frames: string[];
  /** Why there are none, when they could not be worked out. */
  issue?: string;
}

/** Stands in for a cross-site iframe the recorder could not arm. */
const UNARMED = {
  action: 'unsupported_frame',
  captureIssue: 'An embedded frame from another site could not be recorded. Review this part before validation.',
};

/** Why a step inside a cross-site iframe has no frame path. */
const FRAME_ISSUE = 'The frame target could not be identified. Set its frame selector before validation.';

/** The cross-site iframes of one recording channel. */
export class RemoteFrames {
  /** The page's channel. */
  private readonly parent: ParentChannel;
  /** The page's iframe sessions, or null when the transport cannot reach them. */
  private readonly frames: FrameSessions | null;
  /** Builds a channel for one iframe. */
  private readonly makeChannel: MakeChannel;
  /** sessionId → the iframe's channel. */
  readonly children = new Map<string, ChildChannel>();
  /** Stops hearing of new iframes. */
  private unwatch: (() => unknown) | null = null;
  /** Frames waiting to be flagged unless their session attaches first. */
  private readonly pending = new Set<ReturnType<typeof setTimeout>>();

  /** `parent` is the page's channel; `frames` lists, watches and talks to iframe sessions (see src/main/recording/frame-sessions.ts). */
  constructor(parent: ParentChannel, frames: FrameSessions | null, makeChannel: MakeChannel) {
    this.parent = parent;
    this.frames = frames;
    this.makeChannel = makeChannel;
  }

  /**
   * `flag` a frame the page's own session could not arm, unless it is a cross-site
   * iframe whose session attaches within a moment: one nested in a component's
   * shadow root attaches after the frame tree lists it.
   */
  flagUnlessArmed(frameId: string, flag: () => unknown): unknown {
    if (!this.frames) return flag();
    const timer = setTimeout(() => {
      this.pending.delete(timer);
      if (!this.owns(frameId)) flag();
    }, RECORDING.FRAME_ATTACH_GRACE_MS);
    this.pending.add(timer);
  }

  /** Arms every iframe on the page now, and each one that attaches while recording. */
  async armAll(): Promise<void> {
    if (!this.frames) return;
    this.unwatch = this.frames.watch((sessionId, frameId) => this.arm(sessionId, frameId));
    for (const { sessionId, frameId } of this.frames.list()) await this.arm(sessionId, frameId);
  }

  /** Arms one iframe's session; one that refuses becomes a step flagged for review. */
  async arm(sessionId: string, frameId: string): Promise<void> {
    if (this.children.has(sessionId) || !this.frames) return;
    const port = this.frames.port(sessionId);
    const child = this.makeChannel({ ...port, receive: (out) => this.forward(out, frameId) });
    this.children.set(sessionId, child);
    await child.start().catch(() => {
      this.children.delete(sessionId);
      this.parent.receive({ steps: [{ ...UNARMED }] });
    });
  }

  /** Hands the iframe's steps on, each with the iframe's place in the page before its own frames. */
  async forward(out: RecorderOutput, frameId: string): Promise<unknown> {
    if (!out.steps?.length) return this.parent.receive(out);
    const { frames, issue } = await this.ownerPath(frameId);
    const tag = (step: RecordedStep): RecordedStep => ({
      ...step,
      frames: [...frames, ...(step.frames || [])],
      ...(issue ? { captureIssue: issue } : {}),
    });
    this.parent.receive({ ...out, steps: out.steps.map(tag) });
  }

  /**
   * The frame selectors from the page down to the iframe, or none and why. A
   * cross-site iframe can be missing from the page's own frame tree; its owner
   * element, asked for directly, is then found in the top document.
   */
  async ownerPath(frameId: string): Promise<Located> {
    const direct = () => frameSelector(this.parent.send, frameId).then((selector) => [selector]);
    try {
      return { frames: await this.parent.framePath(frameId).catch(direct) };
    } catch {
      return { frames: [], issue: FRAME_ISSUE };
    }
  }

  /** Whether `frameId` is a cross-site iframe this recording reaches through its own session. */
  owns(frameId: string): boolean {
    return !!this.frames?.list().some((f) => f.frameId === frameId);
  }

  /** Runs `work` on every iframe's channel; a gone iframe is dropped quietly. */
  async each(work: (child: ChildChannel) => Promise<unknown>): Promise<void> {
    for (const [sessionId, child] of this.children) {
      await work(child).catch(() => this.children.delete(sessionId));
    }
  }

  /** Stops every iframe's channel and stops listening for new ones. */
  async stop(): Promise<void> {
    this.unwatch?.();
    this.unwatch = null;
    for (const timer of this.pending) clearTimeout(timer);
    this.pending.clear();
    await this.each((child) => child.stop());
    this.children.clear();
  }
}
