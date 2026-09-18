/**
 * Recording channel: captures a person's actions on one page over CDP. The
 * analyzer's recorder runs in a dedicated isolated world in the page and in
 * every frame, reports through a private binding, and each reported step is
 * tagged with the frame path it happened in.
 *
 * Used by the desktop app (Electron's debugger) and the server's CDP driver:
 * `send(method, params)` → Promise and `on(event, handler)` → unsubscribe.
 */
const { randomBytes } = require('node:crypto');
const { RECORDING } = require('./constants.cjs');
const { recorderSource, STOP_EXPRESSION } = require('./recording/source.cjs');
const { framePath } = require('./recording/frames.cjs');

/** Marks a binding payload that was not JSON. */
const NOT_JSON = Symbol('not JSON');

/** Stands in for a frame the recorder could not reach. */
const UNSUPPORTED_FRAME = {
  action: 'unsupported_frame',
  captureIssue: 'An embedded frame could not be recorded. Review this part of the workflow before validation.',
};

/** Why a step's frame path is missing, when it could not be worked out. */
const FRAME_ISSUE = 'The frame target could not be identified. Set its frame selector before validation.';

/** A binding payload parsed, or NOT_JSON. */
function parseMessage(payload) {
  try {
    return JSON.parse(payload);
  } catch {
    return NOT_JSON;
  }
}

/** A step's frame path, or no frames and the reason why. */
async function locateFrames(channel, frameId) {
  try {
    return { frames: await channel.framePath(frameId) };
  } catch {
    return { frames: [], issue: FRAME_ISSUE };
  }
}

/** Waits for `promise`, rejecting if the page does not connect in time. */
function withTimeout(promise) {
  let timeout;
  const failure = () => new Error('Page recording did not connect. Restart this browser and try again.');
  const expire = new Promise(
    (_, reject) => (timeout = setTimeout(() => reject(failure()), RECORDING.READY_TIMEOUT_MS)),
  );
  return Promise.race([promise, expire]).finally(() => clearTimeout(timeout));
}

/** Capture in a dedicated isolated world. In Electron, createIsolatedWorld can
 * return a different context from the same-named new-document script's world;
 * keep the actual recorder contexts instead of using the analyzer's evaluator. */
class RecordingChannel {
  /** `receive` gets each batch of steps; `disableRuntimeOnStop` turns Runtime off on stop (when nothing else uses it). */
  constructor({ send, on, worldName, analyzer, receive, disableRuntimeOnStop = false }) {
    Object.assign(this, { send, on, analyzer, receive, disableRuntimeOnStop });
    this.binding = 'r' + randomBytes(RECORDING.BINDING_BYTES).toString('hex');
    this.worldName = worldName + '-' + this.binding;
    this.contexts = new Set();
    this.contextFrames = new Map();
    this.delivery = Promise.resolve();
    this.listeners = [];
  }

  /** Evaluates `expression` in one recorder context and returns its value. */
  async evaluate(contextId, expression) {
    const params = { contextId, expression, returnByValue: true, awaitPromise: true };
    const result = await this.send('Runtime.evaluate', params);
    if (result.exceptionDetails) {
      throw new Error(result.exceptionDetails.exception?.description || 'Recorder evaluation failed');
    }
    return result.result?.value;
  }

  /** Arms the recorder in the page and every frame, and waits until it answers; stops cleanly on failure. */
  async start() {
    const { frameTree } = await this.send('Page.getFrameTree');
    this.frameId = frameTree.frame.id;
    const ready = this.listen();
    await this.arm(frameTree, ready).catch(async (err) => {
      await this.stop().catch(() => {});
      throw err;
    });
  }

  /** The steps of start(): binding, recorder in every world, then the handshake. */
  async arm(frameTree, ready) {
    await this.enableBinding();
    const source = recorderSource(this.binding, this.analyzer);
    const executionContextId = await this.armPage(frameTree, source);
    await this.handshake(executionContextId, ready);
  }

  /** Follows recorder contexts and binding calls; resolves when the recorder says it is ready. */
  listen() {
    let acknowledge;
    const ready = new Promise((resolve) => (acknowledge = resolve));
    const on = (event, handler) => this.listeners.push(this.on(event, handler));
    on('Runtime.executionContextCreated', ({ context }) => this.contextCreated(context));
    on('Runtime.executionContextDestroyed', ({ executionContextId }) => this.contexts.delete(executionContextId));
    on('Runtime.executionContextsCleared', () => this.contexts.clear());
    on('Runtime.bindingCalled', (event) => this.bindingCalled(event, acknowledge));
    return ready;
  }

  /** A new context in the recorder's world: remember it and its frame. */
  contextCreated(context) {
    if (context.name === this.worldName) this.track(context.id, context.auxData?.frameId);
  }

  /** Remembers a recorder context and the frame it belongs to. */
  track(contextId, frameId) {
    this.contexts.add(contextId);
    this.contextFrames.set(contextId, frameId);
  }

  /** A message from the recorder: the ready answer, or steps to deliver. */
  bindingCalled(event, acknowledge) {
    if (event.name !== this.binding) return;
    const data = parseMessage(event.payload);
    if (data === NOT_JSON) return;
    if (data.ready === this.binding) acknowledge();
    else this.deliver(data, event.executionContextId);
  }

  /** Enables Runtime and adds the private binding the recorder reports through. */
  async enableBinding() {
    // Electron 35 does not deliver bindingCalled until Runtime is enabled.
    // Only desktop recording owns this subscription; the CDP driver already
    // keeps Runtime enabled for the lifetime of its connection.
    await this.send('Runtime.enable');
    this.runtimeStarted = true;
    await this.send('Runtime.addBinding', { name: this.binding, executionContextName: this.worldName });
  }

  /** Installs the recorder for new documents, arms the current page and its frames; returns the page's context. */
  async armPage(frameTree, source) {
    const worldName = this.worldName;
    const { identifier } = await this.send('Page.addScriptToEvaluateOnNewDocument', { source, worldName });
    this.script = identifier;
    const { executionContextId } = await this.send('Page.createIsolatedWorld', { frameId: this.frameId, worldName });
    this.track(executionContextId, this.frameId);
    await this.evaluate(executionContextId, source);
    await this.armChildren(frameTree, source);
    return executionContextId;
  }

  /** Arms every frame under `tree`, depth first. */
  async armChildren(tree, source) {
    for (const child of tree.childFrames || []) {
      await this.armFrame(child.frame.id, source);
      await this.armChildren(child, source);
    }
  }

  /** Arms one frame; a frame that cannot be armed becomes a step flagged for review. */
  async armFrame(frameId, source) {
    try {
      const world = await this.send('Page.createIsolatedWorld', { frameId, worldName: this.worldName });
      this.track(world.executionContextId, frameId);
      await this.evaluate(world.executionContextId, source);
    } catch {
      this.receive({ steps: [{ ...UNSUPPORTED_FRAME }] });
    }
  }

  /** Pings the binding from the page and waits for the answer to come back. */
  async handshake(executionContextId, ready) {
    const binding = JSON.stringify(this.binding);
    await this.evaluate(
      executionContextId,
      `window[${binding}](${JSON.stringify(JSON.stringify({ ready: this.binding }))})`,
    );
    await withTimeout(ready);
  }

  /** The selectors from the top frame down to `frameId` ([] for the top frame). */
  framePath(frameId) {
    return framePath(this.send, this.frameId, frameId);
  }

  /** Queues a recorder message for delivery, in order, with each step's frame path. */
  deliver(data, contextId) {
    const frameId = this.contextFrames.get(contextId);
    this.delivery = this.delivery.then(() => this.forward(data, frameId));
    return this.delivery;
  }

  /** Hands a message to `receive`, tagging its steps with their frames (or why they have none). */
  async forward(data, frameId) {
    if (!data.steps?.length) return this.receive(data);
    const { frames, issue } = await locateFrames(this, frameId);
    const tag = (step) => ({ ...step, frames, ...(issue ? { captureIssue: issue } : {}) });
    this.receive({ ...data, steps: data.steps.map(tag) });
  }

  /** Evaluates `expression` in every recorder context, delivering whatever each returns. */
  async visit(expression) {
    for (const id of this.contexts) await this.visitContext(id, expression);
  }

  /** One context of visit(); a context navigation destroyed is dropped quietly. */
  async visitContext(id, expression) {
    try {
      const out = await this.evaluate(id, expression);
      if (out) await this.deliver(out, id);
    } catch (err) {
      // Navigation can destroy a context while a status/stop is in flight.
      if (!/context|Cannot find/i.test(err.message || '')) throw err;
      this.contexts.delete(id);
    }
  }

  /** Collects the steps recorded so far (`final` flushes a pending input too). */
  drain(final = false) {
    return this.visit(`window.__acRecordDrain?.(${!!final})`);
  }

  /** Discards what the recorders hold. */
  clear() {
    return this.visit('window.__acRecordClear?.(); undefined');
  }

  /** Stops recording everywhere and releases the binding, listeners and Runtime. */
  async stop() {
    try {
      if (this.script) await this.send('Page.removeScriptToEvaluateOnNewDocument', { identifier: this.script });
      await this.visit(STOP_EXPRESSION);
    } finally {
      await this.teardown();
    }
  }

  /** The cleanup stop() always runs, whatever the page did. */
  async teardown() {
    await this.send('Runtime.removeBinding', { name: this.binding }).catch(() => {});
    for (const off of this.listeners) off();
    this.listeners = [];
    await this.delivery;
    this.forget();
    if (this.runtimeStarted && this.disableRuntimeOnStop) await this.send('Runtime.disable').catch(() => {});
    this.runtimeStarted = false;
  }

  /** Forgets every context and the new-document script. */
  forget() {
    this.contexts.clear();
    this.contextFrames.clear();
    this.script = null;
  }
}

module.exports = { RecordingChannel };
