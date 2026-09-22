/**
 * Cross-site iframes in a recording. Each runs in its own process, a separate
 * target with its own CDP session, so the page's channel cannot arm it. Here
 * each gets a channel of its own on its session, and its steps come out with
 * the iframe's place in the page in front of their own frame path.
 *
 * Optional: a transport that cannot talk to child sessions passes no `frames`,
 * and those iframes stay unrecorded as before.
 */

const { frameSelector } = require('./frames.cjs');
const { RECORDING } = require('../constants.cjs');

/** Stands in for a cross-site iframe the recorder could not arm. */
const UNARMED = {
  action: 'unsupported_frame',
  captureIssue: 'An embedded frame from another site could not be recorded. Review this part before validation.',
};

/** Why a step inside a cross-site iframe has no frame path. */
const FRAME_ISSUE = 'The frame target could not be identified. Set its frame selector before validation.';

/** The cross-site iframes of one recording channel. */
class RemoteFrames {
  /** `parent` is the page's channel; `frames` lists, watches and talks to iframe sessions (see main/recording/frame-sessions.cjs). */
  constructor(parent, frames, makeChannel) {
    Object.assign(this, { parent, frames, makeChannel });
    /** sessionId → the iframe's channel. */
    this.children = new Map();
    /** Stops hearing of new iframes. */
    this.unwatch = null;
    /** Frames waiting to be flagged unless their session attaches first. */
    this.pending = new Set();
  }

  /**
   * `flag` a frame the page's own session could not arm, unless it is a cross-site
   * iframe whose session attaches within a moment: one nested in a component's
   * shadow root attaches after the frame tree lists it.
   */
  flagUnlessArmed(frameId, flag) {
    if (!this.frames) return flag();
    const timer = setTimeout(() => {
      this.pending.delete(timer);
      if (!this.owns(frameId)) flag();
    }, RECORDING.FRAME_ATTACH_GRACE_MS);
    this.pending.add(timer);
  }

  /** Arms every iframe on the page now, and each one that attaches while recording. */
  async armAll() {
    if (!this.frames) return;
    this.unwatch = this.frames.watch((sessionId, frameId) => this.arm(sessionId, frameId));
    for (const { sessionId, frameId } of this.frames.list()) await this.arm(sessionId, frameId);
  }

  /** Arms one iframe's session; one that refuses becomes a step flagged for review. */
  async arm(sessionId, frameId) {
    if (this.children.has(sessionId)) return;
    const port = this.frames.port(sessionId);
    const child = this.makeChannel({ ...port, receive: (out) => this.forward(out, frameId) });
    this.children.set(sessionId, child);
    await child.start().catch(() => {
      this.children.delete(sessionId);
      this.parent.receive({ steps: [{ ...UNARMED }] });
    });
  }

  /** Hands the iframe's steps on, each with the iframe's place in the page before its own frames. */
  async forward(out, frameId) {
    if (!out.steps?.length) return this.parent.receive(out);
    const { frames, issue } = await this.ownerPath(frameId);
    const tag = (step) => ({
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
  async ownerPath(frameId) {
    const direct = () => frameSelector(this.parent.send, frameId).then((selector) => [selector]);
    try {
      return { frames: await this.parent.framePath(frameId).catch(direct) };
    } catch {
      return { frames: [], issue: FRAME_ISSUE };
    }
  }

  /** Whether `frameId` is a cross-site iframe this recording reaches through its own session. */
  owns(frameId) {
    return !!this.frames?.list().some((f) => f.frameId === frameId);
  }

  /** Runs `work` on every iframe's channel; a gone iframe is dropped quietly. */
  async each(work) {
    for (const [sessionId, child] of this.children) {
      await work(child).catch(() => this.children.delete(sessionId));
    }
  }

  /** Stops every iframe's channel and stops listening for new ones. */
  async stop() {
    this.unwatch?.();
    this.unwatch = null;
    for (const timer of this.pending) clearTimeout(timer);
    this.pending.clear();
    await this.each((child) => child.stop());
    this.children.clear();
  }
}

module.exports = { RemoteFrames };
