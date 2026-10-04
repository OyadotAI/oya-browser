/**
 * Validation: runs a draft's generated Playwright module against fresh tabs of
 * this browser. An isolated worker drives only fresh validation tabs, using the
 * installed Chromium, through a run-scoped CDP front door that exposes those
 * tabs and nothing else.
 */
import { randomBytes } from 'node:crypto';
import type { EventEmitter } from 'node:events';
import { once } from 'node:events';
import fs from 'node:fs';
import type { AddressInfo } from 'node:net';
import path from 'node:path';
import type { App } from 'electron';
import type { Draft } from '../../workflow/index.ts';
import type { CloseOptions, DoorTab, FrontDoorOptions } from '../front-door/types.ts';
import { withinTime } from '../../shared/within-time.ts';
import { VALIDATION } from './constants.ts';
import type { WorkerMessage } from './runs.ts';

/** How a person asked the run to go. */
export interface RunOptions {
  /** Variable values by name. */
  vars?: Record<string, unknown>;
  /** 'step' to stop after the first step. */
  command?: string;
  /** The step to pause at. */
  runTo?: string;
  /** Whether to collect evidence. */
  evidence?: boolean;
  /** False to turn off auto-heal. */
  autoHeal?: boolean;
  /** A pause before each step, in milliseconds. */
  slowMo?: number | string;
}

/** One of the desktop's tabs, as a run sees it. */
export interface ValidationTab extends DoorTab {
  /** The tab whose page opened this one. */
  openerId?: number;
}

/** The Playwright worker, as a utility process. */
export interface WorkerProcess {
  /** Hears the worker's messages and its exit. */
  on(event: 'message' | 'exit', listener: (message: WorkerMessage) => void): unknown;
  /** Sends the worker a message. */
  postMessage(message: unknown): void;
  /** Ends the worker. */
  kill(): unknown;
}

/** The front door's server, as far as a run uses it. */
export interface DoorServer extends EventEmitter {
  /** Stops listening. */
  close(): unknown;
  /** Where it listens. */
  address(): AddressInfo | string | null;
}

/** Who holds control, as a run reads it. */
export interface ControlView {
  /** Whether this desktop's person holds control. */
  mine?: boolean;
  /** 'agent' or 'human'. */
  mode?: string;
}

/** Who drives the browser: a run takes control back and admits its commands through it. */
export interface ValidationControl {
  /** The current state: whether a person holds control. */
  snapshot(): ControlView;
  /** Asks for a change of control. */
  change(change: string): Promise<unknown>;
  /** Admits one local automation command. */
  beginLocalCommand: NonNullable<FrontDoorOptions['beginCommand']>;
  /** A local client connected or left. */
  localClient: NonNullable<FrontDoorOptions['clientChanged']>;
}

/** What validate() is given: the draft, options and event sink, and the browser's hooks. */
export interface ValidationDeps {
  /** The draft to run. */
  draft: Draft;
  /** How to run it. */
  options: RunOptions;
  /** Hears the run's messages. */
  event: (message: WorkerMessage) => void;
  /** Electron's app, for its folders. */
  app: Pick<App, 'getPath'>;
  /** Electron's utilityProcess. */
  utilityProcess: { fork(modulePath: string, args: string[], options: object): WorkerProcess };
  /** The built worker to fork. */
  workerPath: string;
  /** Starts a CDP front door (src/main/front-door/cdp-front-door.ts's `start`). */
  startFrontDoor: (options: FrontDoorOptions) => DoorServer;
  /** Who drives the browser. */
  control: ValidationControl;
  /** The desktop's tabs now. */
  tabs: () => ValidationTab[];
  /** Opens a tab the protected way; answers its id. */
  createTab: (url: string) => number;
  /** Closes a tab. */
  closeTab: (id: number, options?: CloseOptions) => void;
  /** The last run's tabs, closed when the next starts. */
  leftOpen?: Set<number>;
  /** Chromium's debugging port, or 0 to read it from the profile. */
  cdpPort?: number;
}

/** What a started run answers: run controls and its disposal. */
export interface ValidationSession {
  /** Pause, resume, step or stop. */
  control(command: string): void;
  /** Stops everything the run started. */
  dispose(): void;
}

/** The address of a validation tab named `name`. */
const tabUrl = (name: string): string => 'about:blank#oya-run-' + (name === 'main' ? 'main' : encodeURIComponent(name));

/** Resolves after `ms`. */
const sleep = (ms: number): Promise<void> => new Promise((resolve) => setTimeout(resolve, ms));

/** Reads Chromium's ephemeral debugging port from its profile, if it has written it yet. */
function readDebugPort(app: Pick<App, 'getPath'>): number | undefined {
  try {
    return Number(fs.readFileSync(path.join(app.getPath('userData'), 'DevToolsActivePort'), 'utf8').split('\n')[0]);
  } catch {
    return undefined;
  }
}

/** Where the worker finds the run: the door's endpoint, its secret, and the scratch folder. */
interface RunAddress {
  /** The front door's HTTP endpoint. */
  endpoint: string;
  /** The run's secret. */
  token: string;
  /** The scratch folder for the generated module. */
  directory: string;
}

/** The run tabs: the main one, and every tab's address by name. */
interface RunTabs {
  /** The main tab. */
  tab: ValidationTab;
  /** Each tab name's address. */
  pageUrls: Record<string, string>;
}

/** The run's start message: the draft, the door and its secret, and the person's options. */
function startMessage(deps: ValidationDeps, run: RunAddress): object {
  const { options } = deps;
  return {
    ...{ type: 'start', draft: deps.draft, endpoint: run.endpoint, token: run.token, directory: run.directory },
    ...{ vars: options.vars || {}, command: options.command, runTo: options.runTo, evidence: !!options.evidence },
    autoHeal: options.autoHeal !== false,
    slowMo: Math.min(Math.max(Number(options.slowMo) || 0, 0), VALIDATION.MAX_SLOW_MO_MS),
  };
}

/** The finished message for a run the worker never finished. */
const interrupted = (error: string): WorkerMessage => ({ type: 'finished', status: 'interrupted', error });

/** One validation run: its tabs, front door, worker and cleanup. */
class ValidationRun {
  /** validate()'s arguments. */
  private readonly deps: ValidationDeps;
  /** The secret the worker presents to the front door. */
  private readonly token = randomBytes(VALIDATION.TOKEN_BYTES).toString('hex');
  /** CDP target ids the worker may reach. */
  private readonly targets = new Set<string>();
  /** Tab ids opened for this run. */
  private readonly runTabs = new Set<number>();
  /** Scratch folder for the generated module. */
  private readonly directory: string;
  /** Set once cleanup has run. */
  private finished = false;
  /** Reports a stop the worker never confirms. */
  private stopTimer: ReturnType<typeof setTimeout> | undefined;
  /** The run-scoped front door. */
  private server: DoorServer | undefined;
  /** The Playwright worker. */
  private worker: WorkerProcess | undefined;

  /** `deps` are validate()'s arguments: the draft, options, event sink and the browser's hooks. */
  constructor(deps: ValidationDeps) {
    this.deps = deps;
    this.directory = fs.mkdtempSync(path.join(deps.app.getPath('temp'), 'oya-validation-'));
  }

  /** Stops everything the run started, once. */
  cleanup(): void {
    if (this.finished) return;
    this.finished = true;
    clearTimeout(this.stopTimer);
    this.server?.close();
    this.worker?.kill();
    fs.rmSync(this.directory, { recursive: true, force: true });
  }

  /**
   * Closes the tabs the last run left open to show where it ended, and the tabs
   * their pages opened, so each run does not add another tab.
   */
  private closeLeftOpen(): void {
    const left = this.deps.leftOpen;
    if (!left?.size) return;
    const stale = this.deps.tabs().filter((t) => left.has(t.id) || left.has(t.openerId!));
    left.clear();
    for (const tab of stale) this.deps.closeTab(tab.id);
  }

  /** Opens a tab for this run and returns it. */
  private addTab(url: string): ValidationTab {
    const id = this.deps.createTab(url);
    this.runTabs.add(id);
    this.deps.leftOpen?.add(id);
    return this.deps.tabs().find((t) => t.id === id)!;
  }

  /** Records the tab's CDP target so the front door lets the worker reach it. */
  private async registerTarget(tab: ValidationTab): Promise<void> {
    const { targetInfo } = await tab.view.webContents.debugger.sendCommand('Target.getTargetInfo');
    tab.targetId = targetInfo.targetId;
    this.targets.add(targetInfo.targetId);
  }

  /** Opens a run tab, waits for it, and registers its target. */
  private async open(url: string): Promise<ValidationTab> {
    const tab = this.addTab(url);
    const opened = Promise.resolve(tab.ready).then(() => this.registerTarget(tab));
    await withinTime(opened, VALIDATION.TAB_OPEN_MS, 'The validation tab did not open. Close some tabs and try again.');
    return tab;
  }

  /** Hands control back from a person, then opens the main tab and one per named tab the steps use. */
  private async prepareTabs(): Promise<RunTabs> {
    const { control } = this.deps;
    if (control.snapshot().mine || control.snapshot().mode === 'human') await control.change('return');
    const tab = await this.open(tabUrl('main'));
    const pageUrls = { main: tabUrl('main') };
    await this.openNamedTabs(pageUrls);
    return { tab, pageUrls };
  }

  /** Opens a tab for every other tab name the enabled steps use, recording each address. */
  private async openNamedTabs(pageUrls: Record<string, string>): Promise<void> {
    for (const name of new Set(this.deps.draft.steps.filter((step) => step.enabled).map((step) => step.tab))) {
      if (name === 'main') continue;
      pageUrls[name] = tabUrl(name);
      await this.open(pageUrls[name]);
    }
  }

  /** The browser's debugging port: given, or waited for until Chromium writes it. */
  private async debugPort(): Promise<number> {
    // Chromium writes the ephemeral debugging port to its profile directory.
    let upstream = this.deps.cdpPort;
    for (let i = 0; !upstream && i < VALIDATION.PORT_POLLS; i++) {
      upstream = readDebugPort(this.deps.app);
      if (!upstream) await sleep(VALIDATION.PORT_POLL_MS);
    }
    if (!upstream) throw new Error('The browser debugging endpoint did not start. Restart Oya Browser.');
    return upstream;
  }

  /** A tab the worker opens: its target is added once it is ready, before Playwright sees it. */
  private createRunTab(url: string): number {
    const tab = this.addTab(url);
    const ready = tab.ready;
    tab.ready = Promise.resolve(ready).then(() => this.registerTarget(tab));
    return tab.id;
  }

  /** The front door's options: only this run's tabs, admitted through the control gate. */
  private doorOptions(upstream: number): FrontDoorOptions {
    const { control, closeTab } = this.deps;
    return {
      ...{ port: 0, upstream, host: '127.0.0.1', runToken: this.token, allowedTarget: (id) => this.targets.has(id) },
      tabs: () => this.deps.tabs().filter((t) => this.runTabs.has(t.id)),
      // The proxy awaits tab.ready; add its target before exposing it to Playwright.
      createTab: (url) => this.createRunTab(url),
      closeTab,
      ...{ beginCommand: () => control.beginLocalCommand(), clientChanged: (d) => control.localClient(d) },
    };
  }

  /** Starts the run-scoped CDP front door and waits until it listens. */
  private async startFrontDoor(upstream: number): Promise<void> {
    this.server = this.deps.startFrontDoor(this.doorOptions(upstream));
    await once(this.server, 'listening');
  }

  /** Relays a worker message, cleaning up once the run has finished. */
  private relay(message: WorkerMessage): void {
    if (this.finished) return;
    try {
      this.deps.event(message);
    } finally {
      if (message.type === 'finished') this.cleanup();
    }
  }

  /** Ends the run with `message` unless it already finished. */
  private interrupt(message: WorkerMessage): void {
    if (this.finished) return;
    this.deps.event(message);
    this.cleanup();
  }

  /** Forks the Playwright worker and wires its messages and exit. */
  private forkWorker(): void {
    const options = { serviceName: 'Oya Playwright validation', stdio: 'pipe' };
    this.worker = this.deps.utilityProcess.fork(this.deps.workerPath, [], options);
    this.worker.on('message', (message) => this.relay(message));
    this.worker.on('exit', () =>
      this.interrupt(interrupted('Validation process exited. No step was automatically resubmitted.')),
    );
  }

  /** Tells the worker to start the run. */
  private startWorker(tab: ValidationTab, pageUrls: Record<string, string>): void {
    const endpoint = `http://127.0.0.1:${(this.server!.address() as AddressInfo).port}`;
    const message = startMessage(this.deps, { endpoint, token: this.token, directory: this.directory });
    this.worker!.postMessage({ ...message, targetId: tab.targetId, pageUrls });
  }

  /** Passes a run control to the worker; a stop that gets no answer is reported after a grace period. */
  private sendControl(command: string): void {
    if (this.finished) return;
    this.worker!.postMessage({ type: 'control', command });
    if (command === 'stop') this.stopTimer = setTimeout(() => this.stopExpired(), VALIDATION.STOP_GRACE_MS);
  }

  /** The worker did not stop in time: report the run as interrupted. */
  private stopExpired(): void {
    this.deps.event(interrupted('Worker stopped. The last website action may have completed; check before retrying.'));
    this.cleanup();
  }

  /** Every step of starting the run, in order. */
  async begin(): Promise<ValidationSession> {
    this.closeLeftOpen();
    const { tab, pageUrls } = await this.prepareTabs();
    await this.startFrontDoor(await this.debugPort());
    this.forkWorker();
    this.startWorker(tab, pageUrls);
    return { control: (command) => this.sendControl(command), dispose: () => this.cleanup() };
  }
}

/** Starts validating a draft; returns `{ control(command), dispose() }`. Any failure cleans up and rethrows. */
export async function validate(deps: ValidationDeps): Promise<ValidationSession> {
  const run = new ValidationRun(deps);
  try {
    return await run.begin();
  } catch (error) {
    run.cleanup();
    throw error;
  }
}
