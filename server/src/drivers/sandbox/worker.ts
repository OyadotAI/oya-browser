/**
 * The sandbox runtimes a cloud browser can run on, and the one place one is
 * chosen. Everything a runtime does not have to know, naming, ownership,
 * quotas, the enrolment contract, the inventory cache, lives outside the
 * runtimes, so each one only translates a spec into its own API.
 *
 * Select with OYA_CLOUD_RUNTIME=daytona|docker|k8s|ecs (default daytona), or
 * per key with the sandbox_runtime setting.
 */
import * as daytona from './workers/daytona.ts';
import * as docker from './workers/docker.ts';
import * as k8s from './workers/k8s.ts';
import * as ecs from './workers/ecs.ts';

/** What a runtime is asked to start: the same for every runtime. */
export type SandboxSpec = {
  /** PREFIX + browser id, how the sandbox is found again. */
  name: string;
  /** Selection and ownership labels; never the API key itself. */
  labels: Record<string, string>;
  /** The environment browser/main.js enrols with. Carries the API key. */
  env: Record<string, string>;
  /** Idle stop, where the runtime has one. */
  ttlMinutes: number;
  /** Hard lifetime, after which the runtime or the image stops the browser. */
  lifetimeMinutes: number;
};

/** What a runtime answers once a sandbox is starting. */
export type CreatedSandbox = {
  /** The runtime's own id for it: a sandbox id, container id, pod UID or task ARN. */
  id: string;
};

/** A sandbox as found upstream: its labels, its state, and how to delete it. */
export type FoundSandbox = {
  /** Labels as written at create time, checked for ownership before any delete. */
  labels: Record<string, string>;
  /** The runtime's own word for its state, shown on a disconnected row. */
  state: string;
  /** Deletes it; already gone is fine. */
  destroy: () => Promise<void>;
};

/** One sandbox runtime. `config` is whatever its `settings` returned, plus the shared settings. */
export type SandboxWorker = {
  /** The name OYA_CLOUD_RUNTIME and sandbox_runtime select it by. */
  id: string;
  /**
   * Whether a key's own settings (and no deployment settings) are a complete
   * account of its own to run on. Always false for a runtime on the operator's
   * hardware, which a key can choose but never configure.
   */
  ownAccount: (env) => boolean;
  /** This runtime's settings from `env`, or null when incomplete. */
  settings: (env) => Record<string, any> | null;
  /** The documented env names this runtime still needs. */
  missing: (env) => string[];
  /** Creates the sandbox and has the browser starting; resolves with the runtime's id for it. */
  create: (config, spec: SandboxSpec) => Promise<CreatedSandbox>;
  /** The sandbox of this name, or null when there is none. */
  find: (config, name: string) => Promise<FoundSandbox | null>;
  /** Every sandbox labelled for this owner; the caller re-checks the labels. */
  list: (config, owner: string) => Promise<Omit<FoundSandbox, 'destroy'>[]>;
};

/** The runtimes, by the name that selects them. */
export const WORKERS: Record<string, SandboxWorker> = { daytona, docker, k8s, ecs };

/** The runtime when none is named. */
export const DEFAULT_RUNTIME = 'daytona';

/** The runtime `env` names, else the default; null for a name that is not a runtime. */
export function workerFor(env): SandboxWorker | null {
  const id = env.OYA_CLOUD_RUNTIME || DEFAULT_RUNTIME;
  return Object.hasOwn(WORKERS, id) ? WORKERS[id] : null;
}
