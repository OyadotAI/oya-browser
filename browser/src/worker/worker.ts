/**
 * The validation worker, run in an Electron utility process: it replays a
 * draft's generated Playwright module against the run's tabs and reports
 * progress to the parent. The parent sends `start` once, then run controls.
 *
 * Behind it: run (the replay loop), hooks (per-step pausing and reporting),
 * target (target checks and auto-heal), locate, identity, evidence and
 * run-state (the controls).
 */
import { redact } from '../workflow/index.ts';
import { RunState } from './run-state.ts';
import { run } from './run.ts';
import type { StartOptions } from './types.ts';

/** A message from the parent: `start` with the run's options, or `control` with a command. */
interface ParentMessage extends Partial<StartOptions> {
  /** start or control. */
  type: string;
  /** The run control, for `control`. */
  command?: string;
}

/** What arrives on the port: Electron wraps the message in `data`. */
interface PortEvent {
  /** The parent's message. */
  data: ParentMessage;
}

/** The port to the parent process: Electron's process.parentPort, or a test's fake. */
export interface ParentPort {
  /** Sends a message to the parent. */
  postMessage(message: unknown): void;
  /** Hears the parent's messages. */
  on(event: 'message', listener: (event: PortEvent) => void): unknown;
}

/** Replays one draft for the parent on the other end of a port, and applies its run controls. */
export class WorkflowWorker {
  /** This run's controls and progress, shared with the replay. */
  private readonly state = new RunState();
  /** The port to the parent. */
  private readonly port: ParentPort;

  /** Listens on `port` from now on. */
  constructor(port: ParentPort) {
    this.port = port;
    port.on('message', ({ data }) => this.receive(data));
  }

  /** Parent message type → what the worker does. */
  private readonly messages: Record<string, (data: ParentMessage) => void> = {
    start: (data) => this.start(data as StartOptions),
    control: (data) => this.state.control(String(data.command)),
  };

  /** Handles one parent message; an unknown type is ignored. */
  private receive(data: ParentMessage): void {
    if (Object.hasOwn(this.messages, data.type)) this.messages[data.type](data);
  }

  /** Sends a message to the parent process. */
  private readonly tell = (message: Record<string, unknown>): void => this.port.postMessage(message);

  /** Starts the run; a failure before any step reports the run finished. */
  private start(options: StartOptions): void {
    run(options, this.state, this.tell).catch((error: Error) =>
      this.tell({ type: 'finished', status: this.state.stopped ? 'stopped' : 'failed', error: redact(error.message) }),
    );
  }
}
