/**
 * The persona in the browser's service and shared workers. They belong to the
 * browser rather than to a tab, so no tab's debugger reaches them, and they read
 * the real machine: CreepJS found a page saying Apple M1 with 6 cores beside its
 * service worker saying Apple M4 with 10, and its hasBadWebGL check compares the
 * two. Covered here over Chromium's own browser-level endpoint (the loopback
 * remote-debugging port src/main/main.ts always opens), with the applier the tabs use.
 */
import fs from 'node:fs';
import path from 'node:path';
import type { AppServices } from '../app/services.ts';
import { CdpWs, type CdpListener } from '../mirror/cdp-ws.ts';
import { createPersonaApplier } from '../../anonymity/apply.ts';
import type { PersonaOptions } from './protection.ts';

/** The services worker coverage uses. */
type Deps = Pick<AppServices, 'protection'>;

/** Chromium's debugging port as it wrote it into its profile, or null before it has. */
export function debugPort(userData: string): number | null {
  try {
    return Number(fs.readFileSync(path.join(userData, 'DevToolsActivePort'), 'utf8').split('\n')[0]) || null;
  } catch {
    return null;
  }
}

/** Said, never thrown: a worker left uncovered reads the real machine, and that should show in the log. */
const workerCoverageFailed = (what: string, err: unknown): void => {
  console.error('[anonymity] worker ' + what + ' failed:', (err as Error | undefined)?.message || err);
};

/** Keeps the active persona on every service and shared worker, over one browser-level connection. */
export class WorkerCoverage {
  /** Says how the active persona is applied. */
  private readonly deps: Deps;
  /** Where DevToolsActivePort is, asked when coverage starts. */
  private readonly userData: () => string;
  /** The browser-level connection, while covering. */
  private cdp: CdpWs | null = null;

  /** `deps` are the main-process services; `userData()` is the folder Chromium writes its port into. */
  constructor(deps: Deps, userData: () => string) {
    this.deps = deps;
    this.userData = userData;
  }

  /** Covers the active persona's workers from now on, replacing earlier coverage. Never throws. */
  async cover(): Promise<void> {
    this.stop();
    const persona = this.deps.protection.personaOptions();
    const port = debugPort(this.userData());
    if (!persona || !port) return;
    await this.connect(port, persona).catch((e) => workerCoverageFailed('coverage', e));
  }

  /** Opens the connection and has Chromium hold each new worker until the persona is on it. */
  private async connect(port: number, persona: PersonaOptions): Promise<void> {
    const cdp = new CdpWs(port);
    this.cdp = cdp;
    await cdp.connect();
    const send = (method: string, params?: object, sessionId?: string): Promise<unknown> =>
      cdp.send(method, params, sessionId);
    const on = (event: string, fn: CdpListener): void => cdp.on(event, fn);
    await createPersonaApplier({ send, on, ...persona, onError: workerCoverageFailed }).browser();
  }

  /** Ends coverage; Chromium resumes whatever the closed connection was holding. */
  stop(): void {
    this.cdp?.close();
    this.cdp = null;
  }
}
