/**
 * Running other programs, and small probes of the machine: versions, ports.
 */
import { spawn } from 'node:child_process';
import { createConnection, type Socket } from 'node:net';
import { PORT_PROBE_MS } from './constants.ts';

/** Where a command runs, and whether its output is shown. */
export interface RunOptions {
  /** Working directory. */
  cwd: string;
  /** Hide its output. */
  quiet?: boolean;
}

/** Resolves on exit code 0, rejects with the command line otherwise. */
function settle(cmd: string, args: string[], resolve: () => void, reject: (e: Error) => void) {
  return (code: number | null) =>
    code === 0 ? resolve() : reject(new Error(`${cmd} ${args.join(' ')} exited ${code}`));
}

/** Streams, because `npm ci` and `docker build` are slow enough that silence reads as a hang. */
export function run(cmd: string, args: string[], opts: RunOptions = { cwd: process.cwd() }): Promise<void> {
  return new Promise((resolve, reject) => {
    const child = spawn(cmd, args, { cwd: opts.cwd, stdio: opts.quiet ? 'ignore' : 'inherit' });
    child.on('error', reject);
    child.on('close', settle(cmd, args, resolve, reject));
  });
}

/** kubectl reads manifests on stdin, so nothing sensitive lands in argv. */
export function pipeTo(cmd: string, args: string[], stdin: string, cwd: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const child = spawn(cmd, args, { cwd, stdio: ['pipe', 'inherit', 'inherit'] });
    child.on('error', reject);
    child.on('close', settle(cmd, args, resolve, reject));
    child.stdin.end(stdin);
  });
}

/** A command's trimmed stdout, or null when it fails or cannot run. */
export async function capture(cmd: string, args: string[], cwd = process.cwd()): Promise<string | null> {
  return new Promise((resolve) => {
    const child = spawn(cmd, args, { cwd, stdio: ['ignore', 'pipe', 'ignore'] });
    const out: string[] = [];
    child.stdout.on('data', (d) => out.push(String(d)));
    child.on('error', () => resolve(null));
    child.on('close', (code) => resolve(code === 0 ? out.join('').trim() : null));
  });
}

/** Whether a program is installed. */
export const has = async (cmd: string) => (await capture(cmd, ['--version'])) !== null;

/** Compare a leading "x.y[.z]" against a floor. Digit-wise, so 5.0 beats 2.24. */
export function atLeast(version: string | null, major: number, minor: number): boolean {
  const m = /(\d+)\.(\d+)/.exec(version || '');
  if (!m) return false;
  return Number(m[1]) > major || (Number(m[1]) === major && Number(m[2]) >= minor);
}

/** Settles a port probe: busy on connect, free on error or timeout. */
function watchProbe(socket: Socket, resolve: (free: boolean) => void): void {
  const settle = (free: boolean) => {
    socket.destroy();
    resolve(free);
  };
  socket.on('connect', () => settle(false)).on('error', () => resolve(true));
  setTimeout(() => settle(true), PORT_PROBE_MS);
}

/** True when nothing answers on the port locally. */
export function portFree(port: number): Promise<boolean> {
  return new Promise((resolve) => watchProbe(createConnection({ port, host: '127.0.0.1' }), resolve));
}
