/** Own deployment subprocesses, bounding probes and keeping failures free of argument values. */
import { spawn, type ChildProcess, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { DEPLOY_PROBE_MS, DEPLOY_PROBE_BYTES, DEPLOY_KILL_MS } from './constants.ts';

/** A probe and its credential helpers share a disposable process group. */
function startProbe(command: string, args: string[]): ChildProcessWithoutNullStreams {
  return spawn(command, args, { stdio: 'pipe', detached: process.platform !== 'win32' });
}

/** Bound both process lifetime and retained output, even when descendants keep pipes open. */
class Probe {
  /** The inspection process and its owned helpers. */
  child: ChildProcessWithoutNullStreams;
  /** One promise settlement, independent of pipe closure. */
  resolve: (value: string | null) => void;
  /** Only bounded stdout is retained. */
  output = '';
  /** Timeout/error/close may race; the first outcome owns cleanup. */
  settled = false;
  /** The hard deadline is released on every outcome. */
  timer: ReturnType<typeof setTimeout>;
  constructor(child: ChildProcessWithoutNullStreams, resolve: (value: string | null) => void, timeoutMs: number) {
    this.child = child;
    this.resolve = resolve;
    this.timer = setTimeout(() => this.finish(null), timeoutMs);
  }
  /** Drain stderr and close stdin so a probe cannot request interactive input. */
  watch(): void {
    this.child.stdin.end();
    this.child.stderr.resume();
    this.child.stdout.on('data', (chunk) => this.append(String(chunk)));
    this.child.on('error', () => this.finish(null));
    this.child.on('close', (code) => this.finish(code === 0 ? this.output.trim() : null));
  }
  /** Enforce the output ceiling without waiting for a noisy process to exit. */
  append(chunk: string): void {
    this.output += chunk;
    if (this.output.length > DEPLOY_PROBE_BYTES) this.finish(null);
  }
  /** Destroy pipes and stop helpers before resolving, rather than depending on a future close event. */
  finish(value: string | null): void {
    if (this.settled) return;
    this.settled = true;
    clearTimeout(this.timer);
    terminate(this.child.pid, 'SIGKILL');
    this.child.stdin.destroy();
    this.child.stdout.destroy();
    this.child.stderr.destroy();
    this.resolve(value);
  }
}

/** A failed or timed-out probe is unavailable evidence; a test can select a shorter deadline. */
export function deploymentProbe(command: string, args: string[], timeoutMs = DEPLOY_PROBE_MS): Promise<string | null> {
  return new Promise((resolve) => new Probe(startProbe(command, args), resolve, timeoutMs).watch());
}

/** Signal an owned process group; Windows only supports signaling the owned child. */
function terminate(pid: number | undefined, signal: NodeJS.Signals): void {
  if (!pid) return;
  try {
    process.kill(process.platform === 'win32' ? pid : -pid, signal);
  } catch {
    /* Already exited. */
  }
}

/** One owned child group and its signal-handler lifetime. */
class ScriptSignals {
  /** Cancellation must still fail if a script handles TERM by exiting zero. */
  cancelled = false;
  /** The script and its children belong to this group. */
  child: ChildProcess;
  /** Give cleanup a bounded grace period. */
  timer: ReturnType<typeof setTimeout> | undefined;
  constructor(child: ChildProcess) {
    this.child = child;
    process.once('SIGINT', this.stop);
    process.once('SIGTERM', this.stop);
  }
  /** Forward interruption, then stop any processes which ignored it. */
  stop = (): void => {
    this.cancelled = true;
    this.child.kill('SIGTERM');
    this.timer ??= setTimeout(() => terminate(this.child.pid, 'SIGKILL'), DEPLOY_KILL_MS);
  };
  /** Release process-global handlers once this script ends. */
  close(): void {
    clearTimeout(this.timer);
    if (this.cancelled) terminate(this.child.pid, 'SIGKILL');
    process.removeListener('SIGINT', this.stop);
    process.removeListener('SIGTERM', this.stop);
  }
}

/** Stream useful build/deploy output without a shell interpreting any supplied option. */
function startScript(script: string, args: string[], cwd: string, env: NodeJS.ProcessEnv): ChildProcess {
  return spawn('bash', [script, ...args], { cwd, env, stdio: 'inherit', detached: process.platform !== 'win32' });
}

/** Success and failure both release signal handlers, and failures omit command arguments. */
function watchScript(child: ChildProcess, operation: string, resolve: () => void, reject: (err: Error) => void): void {
  const signals = new ScriptSignals(child);
  child.on('error', () => reject(startFailure(signals, operation)));
  child.on('close', (code, signal) => {
    signals.close();
    if (code === 0 && !signals.cancelled) resolve();
    else reject(scriptFailure(operation, signals.cancelled, code, signal));
  });
}

/** A failed spawn must release its process-global cancellation handlers too. */
function startFailure(signals: ScriptSignals, operation: string): Error {
  signals.close();
  return new Error(`Could not start deployment ${operation}. Check bash and the checkout.`);
}

/** Failure describes the phase and exit rather than echoing arguments or credentials. */
function scriptFailure(operation: string, cancelled: boolean, code: number | null, signal: string | null): Error {
  return new Error(`Deployment ${operation} failed (${cancelled ? 'cancelled' : signal || `exit ${code}`}).`);
}

/** Forward cancellation to the script's build, cloud CLI and port-forward children. */
export function runDeploymentScript(
  script: string,
  args: string[],
  cwd: string,
  env: NodeJS.ProcessEnv,
): Promise<void> {
  return new Promise((resolve, reject) => watchScript(startScript(script, args, cwd, env), args[0], resolve, reject));
}
