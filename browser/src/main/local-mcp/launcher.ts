/** Launch only known detected agents through native app documents; never submit a task or bypass agent approvals. */
import { setTimeout as sleep } from 'node:timers/promises';
import type { Shell } from 'electron';
import { agentInstallation, agentPlatform, type AgentPlatform } from './agent-platform.ts';
import { LaunchRepository, type LaunchLease } from './launch-repository.ts';
import { AGENT_OPEN_TIMEOUT_MS, AGENT_OPEN_POLL_MS } from './constants.ts';

/** One window's request cannot start unbounded duplicate terminal handoffs. */
export class AgentLauncher {
  /** Native launcher is injected for hermetic tests. */ private readonly shell: Pick<Shell, 'openPath'>;
  /** Platform discovery is captured explicitly, never supplied by the renderer. */ private readonly platform: AgentPlatform;
  /** Private handoff documents are owned by one repository. */ private readonly files: LaunchRepository;
  /** Shutdown cancels unfinished handoffs while leaving opened agents running. */ private readonly active = new Map<
    string,
    AbortController
  >();
  /** Compose launch, discovery and files once at the application root. */
  constructor(shell: Pick<Shell, 'openPath'>, platform = agentPlatform(), files = new LaunchRepository()) {
    this.shell = shell;
    this.platform = platform;
    this.files = files;
  }
  /** Stable IDs are the only launch input that crosses the shell IPC boundary. */
  async open(id: string): Promise<void> {
    const job = this.begin(id);
    try {
      await this.launch(id, job);
    } finally {
      this.active.delete(id);
      job.abort.abort();
    }
  }
  /** Only a supported detected installation can own a bounded handoff. */
  private begin(id: string) {
    if (this.active.has(id)) throw Error('This agent is already opening.');
    const command = agentInstallation(id, this.platform);
    if (!command || !['darwin', 'win32'].includes(this.platform.platform))
      throw Error('Open this agent manually, then paste the test prompt.');
    const abort = new AbortController();
    this.active.set(id, abort);
    return { command, abort };
  }
  /** Temporary handoffs are cleaned on native refusal, timeout, cancellation and success. */
  private async launch(id: string, job: ReturnType<AgentLauncher['begin']>): Promise<void> {
    const lease = ['cursor', 'vscode'].includes(id) ? null : this.files.prepare(job.command, this.platform.platform);
    const timer = setTimeout(() => job.abort.abort(), AGENT_OPEN_TIMEOUT_MS);
    try {
      await this.handoff(lease, job);
    } finally {
      clearTimeout(timer);
      lease?.close();
    }
  }
  /** CLI documents get a real console; editor applications open directly. */
  private async handoff(lease: LaunchLease | null, job: ReturnType<AgentLauncher['begin']>): Promise<void> {
    await this.openPath(lease?.file || job.command.file, job.abort.signal);
    if (lease) await started(lease, job.abort.signal);
  }
  /** An OS refusal is actionable; successful opening is not a passed browser test. */
  private async openPath(file: string, signal: AbortSignal): Promise<void> {
    const message = await abortable(this.shell.openPath(file), signal);
    if (message) throw Error('Could not open the agent. Open it manually, then paste the test prompt.');
  }
  /** Only unfinished temporary handoffs are cancelled when Oya closes. */
  dispose(): void {
    for (const abort of this.active.values()) abort.abort();
  }
}
/** A never-settling native opener must not retain the launch UI or temporary scripts forever. */
function abortable<T>(work: Promise<T>, signal: AbortSignal): Promise<T> {
  return new Promise((resolve, reject) => {
    const abort = () => reject(Error('The agent did not open. Open it manually, then paste the test prompt.'));
    signal.addEventListener('abort', abort, { once: true });
    void work.then(resolve, reject).finally(() => signal.removeEventListener('abort', abort));
    if (signal.aborted) abort();
  });
}
/** Polling is local, cancellable and bounded by the launch deadline. */
async function started(lease: LaunchLease, signal: AbortSignal): Promise<void> {
  while (!lease.started()) {
    signal.throwIfAborted();
    await pause(signal);
  }
}
/** Native promise timers remove their listener and timer immediately on cancellation. */
function pause(signal: AbortSignal): Promise<void> {
  return sleep(AGENT_OPEN_POLL_MS, undefined, { signal });
}
