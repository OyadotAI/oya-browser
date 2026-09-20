/**
 * The kubectl CLI. Manifests go in on stdin, so no credential ever appears in
 * argv (where `ps` would show it). execFile cannot write stdin, hence spawn.
 */
import { spawn, type ChildProcess } from 'node:child_process';
import { CLI_TIMEOUT_MS } from './constants.ts';

/** Runs kubectl (under OYA_K8S_CONTEXT when set) and resolves its stdout; rejects with its stderr attached. */
export function kubectl(args, stdin = null): Promise<string> {
  const base = process.env.OYA_K8S_CONTEXT ? ['--context', process.env.OYA_K8S_CONTEXT] : [];
  return new Promise((resolve, reject) => void new KubectlRun([...base, ...args], stdin, resolve, reject));
}

/** One kubectl process: collects its output and settles once it exits, fails or times out. */
class KubectlRun {
  /** The process. */
  declare private readonly child: ChildProcess;
  /** Its stdout so far. */
  private out = '';
  /** Its stderr so far. */
  private err = '';
  /** Settles the caller's promise with stdout. */
  declare private readonly resolve: (out: string) => void;
  /** Settles the caller's promise with an error. */
  declare private readonly reject: (error: Error) => void;

  /** Starts kubectl with `args`, writing `stdin` (if any) and closing it. */
  constructor(args, stdin, resolve, reject) {
    this.resolve = resolve;
    this.reject = reject;
    this.child = spawn('kubectl', args, { stdio: ['pipe', 'pipe', 'pipe'] });
    this.collect();
    const timer = setTimeout(() => this.timeout(), CLI_TIMEOUT_MS);
    this.child.on('close', (code) => this.close(code, timer));
    if (stdin !== null) this.child.stdin.end(stdin);
    else this.child.stdin.end();
  }

  /** Buffers output and turns a spawn failure into a rejection. */
  private collect() {
    this.child.stdout.on('data', (d) => {
      this.out += d;
    });
    this.child.stderr.on('data', (d) => {
      this.err += d;
    });
    this.child.on('error', (e) => this.reject(Object.assign(e, { stderr: this.err })));
  }

  /** Too slow: kill it and reject. */
  private timeout() {
    this.child.kill('SIGKILL');
    this.reject(Object.assign(new Error('kubectl timed out'), { stderr: this.err }));
  }

  /** Exited: stdout on success, else stderr (or the exit code) as the error. */
  private close(code, timer) {
    clearTimeout(timer);
    if (code === 0) this.resolve(this.out);
    else this.reject(Object.assign(new Error(this.err.trim() || `kubectl exited ${code}`), { stderr: this.err, code }));
  }
}
