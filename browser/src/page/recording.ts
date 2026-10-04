/**
 * Recording channel: captures a person's actions on one page over CDP. The
 * analyzer's recorder runs in a dedicated isolated world in the page and in
 * every frame, reports through a private binding, and each reported step is
 * tagged with the frame path it happened in.
 *
 * Used by the desktop app (Electron's debugger) and the server's CDP driver:
 * `send(method, params)` → Promise and `on(event, handler)` → unsubscribe.
 */
import { randomBytes } from 'node:crypto';
import { RECORDING } from './constants.ts';
import { recorderSource, STOP_EXPRESSION } from './recording/source.ts';
import { framePath, type FrameTree, type FrameTreeResult, type Send } from './recording/frames.ts';
import {
  RemoteFrames,
  type ChildPort,
  type Located,
  type FrameSessions,
  type On,
  type Port,
  type Receive,
  type RecordedStep,
  type RecorderOutput,
} from './recording/remote-frames.ts';

export type { FrameSessions, On, Port, Receive, RecordedStep, RecorderOutput, Send };

/** How a channel is built: its transport, its world, the analyzer, and where its steps go. */
export interface RecordingChannelOptions extends Port {
  /** The isolated world's name; the channel adds its binding to it. */
  worldName: string;
  /** The analyzer's source, armed in every recorder world. */
  analyzer: string;
  /** Hands on each batch of steps. */
  receive: Receive;
  /** Turns Runtime off on stop (when nothing else uses it). */
  disableRuntimeOnStop?: boolean;
  /** When the transport can reach child sessions: arms cross-site iframes too (recording/remote-frames.ts). */
  frames?: FrameSessions | null;
}

/** The frame an execution context belongs to. */
interface ContextAuxData {
  /** The frame's id. */
  frameId?: string;
}

/** An execution context, as Runtime.executionContextCreated reports it. */
interface ContextDescription {
  /** The context's id. */
  id: number;
  /** Its world's name. */
  name: string;
  /** Which frame it belongs to. */
  auxData?: ContextAuxData;
}

/** A Runtime.executionContextCreated event. */
interface ContextCreated {
  /** The new context. */
  context: ContextDescription;
}

/** A Runtime.bindingCalled event. */
interface BindingCall {
  /** The binding's name. */
  name: string;
  /** What the page passed. */
  payload: string;
  /** The context that called it. */
  executionContextId: number;
}

/** What an evaluation threw. */
interface ExceptionDetails {
  /** The thrown value. */
  exception?: {
    /** Its description, with the message. */
    description?: string;
  };
}

/** What Runtime.evaluate answers. */
interface EvaluateResult {
  /** The value, when it did not throw. */
  result?: {
    /** The value itself. */
    value?: unknown;
  };
  /** What it threw. */
  exceptionDetails?: ExceptionDetails;
}

/** What Page.createIsolatedWorld answers. */
interface WorldResult {
  /** The new world's context. */
  executionContextId: number;
}

/** A recorder's message: its ready answer, or what it recorded. */
interface RecorderMessage extends RecorderOutput {
  /** The binding name, when this is the handshake's answer. */
  ready?: string;
}

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
function parseMessage(payload: string): RecorderMessage | typeof NOT_JSON {
  try {
    return JSON.parse(payload);
  } catch {
    return NOT_JSON;
  }
}

/** A step's frame path, or no frames and the reason why. */
async function locateFrames(channel: RecordingChannel, frameId?: string): Promise<Located> {
  try {
    return { frames: await channel.framePath(frameId) };
  } catch {
    return { frames: [], issue: FRAME_ISSUE };
  }
}

/** Waits for `promise`, rejecting if the page does not connect in time. */
function withTimeout(promise: Promise<unknown>): Promise<unknown> {
  let timeout: ReturnType<typeof setTimeout> | undefined;
  const failure = () => new Error('Page recording did not connect. Restart this browser and try again.');
  const expire = new Promise(
    (_, reject) => (timeout = setTimeout(() => reject(failure()), RECORDING.READY_TIMEOUT_MS)),
  );
  return Promise.race([promise, expire]).finally(() => clearTimeout(timeout));
}

/**
 * A delivery that failed is logged and the chain goes on: left rejected, it would
 * silently skip every later step from the tab and make stop throw.
 */
function deliveryFailed(err: Error): void {
  console.error('[recording] delivery failed:', err.message);
}

/** Capture in a dedicated isolated world. In Electron, createIsolatedWorld can
 * return a different context from the same-named new-document script's world;
 * keep the actual recorder contexts instead of using the analyzer's evaluator. */
export class RecordingChannel {
  /** Sends one CDP command to the page's session. */
  readonly send: Send;
  /** Subscribes to one CDP event of the page's session. */
  readonly on: On;
  /** The analyzer's source. */
  readonly analyzer: string;
  /** Hands on each batch of steps. */
  readonly receive: Receive;
  /** Turns Runtime off on stop. */
  readonly disableRuntimeOnStop: boolean;
  /** The page's cross-site iframes. */
  readonly remote: RemoteFrames;
  /** The private binding the recorder reports through. */
  readonly binding = 'r' + randomBytes(RECORDING.BINDING_BYTES).toString('hex');
  /** This channel's own isolated world. */
  readonly worldName: string;
  /** The recorder contexts alive now. */
  readonly contexts = new Set<number>();
  /** Recorder context → its frame. */
  readonly contextFrames = new Map<number, string | undefined>();
  /** The chain deliveries run on, in order. */
  delivery: Promise<unknown> = Promise.resolve();
  /** Unsubscribes each event listener. */
  listeners: Array<() => unknown> = [];
  /** The top frame's id, once started. */
  frameId?: string;
  /** The new-document script's id, while installed. */
  script: string | null = null;
  /** Whether this channel enabled Runtime. */
  runtimeStarted = false;

  /** `options` is the transport, the world, the analyzer and where steps go (see RecordingChannelOptions). */
  constructor(options: RecordingChannelOptions) {
    const { worldName, analyzer, frames = null } = options;
    this.send = options.send;
    this.on = options.on;
    this.analyzer = analyzer;
    this.receive = options.receive;
    this.disableRuntimeOnStop = options.disableRuntimeOnStop ?? false;
    this.remote = new RemoteFrames(this, frames, (port) => childChannel(port, worldName, analyzer));
    this.worldName = worldName + '-' + this.binding;
  }

  /** Evaluates `expression` in one recorder context and returns its value. */
  async evaluate(contextId: number, expression: string): Promise<unknown> {
    const params = { contextId, expression, returnByValue: true, awaitPromise: true };
    const result = (await this.send('Runtime.evaluate', params)) as EvaluateResult;
    if (result.exceptionDetails) {
      throw new Error(result.exceptionDetails.exception?.description || 'Recorder evaluation failed');
    }
    return result.result?.value;
  }

  /** Arms the recorder in the page and every frame, and waits until it answers; stops cleanly on failure. */
  async start(): Promise<void> {
    const { frameTree } = (await this.send('Page.getFrameTree')) as FrameTreeResult;
    this.frameId = frameTree.frame.id;
    const ready = listen(this);
    await arm(this, frameTree, ready).catch(async (err) => {
      await this.stop().catch(() => {});
      throw err;
    });
    await this.remote.armAll();
  }

  /** A new context in the recorder's world: remember it and its frame. */
  contextCreated(context: ContextDescription): void {
    if (context.name === this.worldName) this.track(context.id, context.auxData?.frameId);
  }

  /** Remembers a recorder context and the frame it belongs to. */
  track(contextId: number, frameId?: string): void {
    this.contexts.add(contextId);
    this.contextFrames.set(contextId, frameId);
  }

  /** A message from the recorder: the ready answer, or steps to deliver. */
  bindingCalled(event: BindingCall, acknowledge: () => void): void {
    if (event.name !== this.binding) return;
    const data = parseMessage(event.payload);
    if (data === NOT_JSON) return;
    if (data.ready === this.binding) acknowledge();
    else this.deliver(data, event.executionContextId);
  }

  /** The selectors from the top frame down to `frameId` ([] for the top frame). */
  framePath(frameId?: string): Promise<string[]> {
    return framePath(this.send, this.frameId, frameId);
  }

  /** Queues a recorder message for delivery, in order, with each step's frame path. */
  deliver(data: RecorderOutput, contextId: number): Promise<unknown> {
    const frameId = this.contextFrames.get(contextId);
    this.delivery = this.delivery.then(() => this.forward(data, frameId)).catch(deliveryFailed);
    return this.delivery;
  }

  /** Hands a message to `receive`, tagging its steps with their frames (or why they have none). */
  async forward(data: RecorderOutput, frameId?: string): Promise<unknown> {
    if (!data.steps?.length) return this.receive(data);
    const { frames, issue } = await locateFrames(this, frameId);
    const tag = (step: RecordedStep): RecordedStep => ({ ...step, frames, ...(issue ? { captureIssue: issue } : {}) });
    this.receive({ ...data, steps: data.steps.map(tag) });
  }

  /** Evaluates `expression` in every recorder context, delivering whatever each returns. */
  async visit(expression: string): Promise<void> {
    for (const id of this.contexts) await this.visitContext(id, expression);
  }

  /** One context of visit(); a context navigation destroyed is dropped quietly. */
  async visitContext(id: number, expression: string): Promise<void> {
    try {
      const out = (await this.evaluate(id, expression)) as RecorderOutput | undefined;
      if (out) await this.deliver(out, id);
    } catch (err) {
      // Navigation can destroy a context while a status/stop is in flight.
      if (!/context|Cannot find/i.test((err as Error).message || '')) throw err;
      this.contexts.delete(id);
    }
  }

  /** Collects the steps recorded so far (`final` flushes a pending input too), cross-site iframes included. */
  async drain(final = false): Promise<void> {
    await this.visit(`window.__acRecordDrain?.(${!!final})`);
    await this.remote.each((child) => child.drain(final));
  }

  /** Discards what the recorders hold. */
  async clear(): Promise<void> {
    await this.visit('window.__acRecordClear?.(); undefined');
    await this.remote.each((child) => child.clear());
  }

  /** Stops recording everywhere and releases the binding, listeners and Runtime. */
  async stop(): Promise<void> {
    try {
      await this.remote.stop();
      if (this.script) await this.send('Page.removeScriptToEvaluateOnNewDocument', { identifier: this.script });
      await this.visit(STOP_EXPRESSION);
    } finally {
      await teardown(this);
    }
  }
}

/** What Page.addScriptToEvaluateOnNewDocument answers. */
interface ScriptResult {
  /** The script's id, for removing it. */
  identifier: string;
}

/** Subscribes `handler` to one of the page's events, kept for teardown; the params are CDP's for that event. */
function follow<T>(channel: RecordingChannel, event: string, handler: (params: T) => void): void {
  channel.listeners.push(channel.on(event, (params) => handler(params as T)));
}

/** Keeps the channel's list of recorder contexts current as the page creates and destroys them. */
function followContexts(channel: RecordingChannel): void {
  follow(channel, 'Runtime.executionContextCreated', (e: ContextCreated) => channel.contextCreated(e.context));
  follow(channel, 'Runtime.executionContextDestroyed', (e: WorldResult) =>
    channel.contexts.delete(e.executionContextId),
  );
  follow(channel, 'Runtime.executionContextsCleared', () => channel.contexts.clear());
}

/** Follows recorder contexts and binding calls; resolves when the recorder says it is ready. */
function listen(channel: RecordingChannel): Promise<void> {
  let acknowledge = () => {};
  const ready = new Promise<void>((resolve) => (acknowledge = resolve));
  followContexts(channel);
  follow(channel, 'Runtime.bindingCalled', (event: BindingCall) => channel.bindingCalled(event, acknowledge));
  return ready;
}

/** The steps of start(): binding, recorder in every world, then the handshake. */
async function arm(channel: RecordingChannel, frameTree: FrameTree, ready: Promise<unknown>): Promise<void> {
  await enableBinding(channel);
  const source = recorderSource(channel.binding, channel.analyzer);
  const executionContextId = await armPage(channel, frameTree, source);
  await handshake(channel, executionContextId, ready);
}

/** Enables Runtime and adds the private binding the recorder reports through. */
async function enableBinding(channel: RecordingChannel): Promise<void> {
  // Electron 35 does not deliver bindingCalled until Runtime is enabled.
  // Only desktop recording owns this subscription; the CDP driver already
  // keeps Runtime enabled for the lifetime of its connection.
  await channel.send('Runtime.enable');
  channel.runtimeStarted = true;
  await channel.send('Runtime.addBinding', { name: channel.binding, executionContextName: channel.worldName });
}

/** Creates the recorder's world in `frameId` and evaluates the recorder there. */
async function armWorld(channel: RecordingChannel, frameId: string | undefined, source: string): Promise<number> {
  const params = { frameId, worldName: channel.worldName };
  const world = (await channel.send('Page.createIsolatedWorld', params)) as WorldResult;
  channel.track(world.executionContextId, frameId);
  await channel.evaluate(world.executionContextId, source);
  return world.executionContextId;
}

/** Installs the recorder for new documents, arms the current page and its frames; returns the page's context. */
async function armPage(channel: RecordingChannel, frameTree: FrameTree, source: string): Promise<number> {
  const script = { source, worldName: channel.worldName };
  const added = (await channel.send('Page.addScriptToEvaluateOnNewDocument', script)) as ScriptResult;
  channel.script = added.identifier;
  const executionContextId = await armWorld(channel, channel.frameId, source);
  await armChildren(channel, frameTree, source);
  return executionContextId;
}

/** Arms every frame under `tree`, depth first. */
async function armChildren(channel: RecordingChannel, tree: FrameTree, source: string): Promise<void> {
  for (const child of tree.childFrames || []) {
    await armFrame(channel, child.frame.id, source);
    await armChildren(channel, child, source);
  }
}

/** Arms one frame; a frame that cannot be armed becomes a step flagged for review. */
async function armFrame(channel: RecordingChannel, frameId: string, source: string): Promise<void> {
  try {
    await armWorld(channel, frameId, source);
  } catch {
    // A cross-site iframe is armed through its own session (remote-frames.ts), not here;
    // it is flagged only if that session never comes.
    channel.remote.flagUnlessArmed(frameId, () => channel.receive({ steps: [{ ...UNSUPPORTED_FRAME }] }));
  }
}

/** Pings the binding from the page and waits for the answer to come back. */
async function handshake(channel: RecordingChannel, contextId: number, ready: Promise<unknown>): Promise<void> {
  const binding = JSON.stringify(channel.binding);
  const ping = `window[${binding}](${JSON.stringify(JSON.stringify({ ready: channel.binding }))})`;
  await channel.evaluate(contextId, ping);
  await withTimeout(ready);
}

/** A recording channel for one cross-site iframe's session, with the page's world and analyzer. */
function childChannel(port: ChildPort, worldName: string, analyzer: string): RecordingChannel {
  return new RecordingChannel({ ...port, worldName, analyzer, disableRuntimeOnStop: true });
}

/** Releases the binding and listeners, forgets every context and the new-document script, and turns Runtime off if it may. */
async function teardown(channel: RecordingChannel): Promise<void> {
  await channel.send('Runtime.removeBinding', { name: channel.binding }).catch(() => {});
  for (const off of channel.listeners) off();
  channel.listeners = [];
  await channel.delivery;
  forget(channel);
  if (channel.runtimeStarted && channel.disableRuntimeOnStop) await channel.send('Runtime.disable').catch(() => {});
  channel.runtimeStarted = false;
}

/** Forgets every context and the new-document script. */
function forget(channel: RecordingChannel): void {
  channel.contexts.clear();
  channel.contextFrames.clear();
  channel.script = null;
}
