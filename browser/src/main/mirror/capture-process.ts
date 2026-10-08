/** Watch capture launches and clean their scratch data only after process exit. */
import fs from 'node:fs';
import path from 'node:path';
import type { ChildProcess } from 'node:child_process';
import {
  LAUNCH_READY_TIMEOUT_MS,
  LAUNCH_POLL_MS,
  CLEANUP_RETRIES,
  CLEANUP_RETRY_MS,
  EXIT_TIMEOUT_MS,
} from './constants.ts';

/** Reads the ephemeral port published by the capture browser. */
function portAt(scratch: string): number | null {
  try {
    return Number(fs.readFileSync(path.join(scratch, 'DevToolsActivePort'), 'utf8').split('\n')[0]) || null;
  } catch {
    return null;
  }
}

/** Watches errors from the moment a child is spawned, including before the first poll. */
export class CaptureProcess {
  /** The launched process. */
  private readonly child: ChildProcess;
  /** A launch or early-exit failure, retained until the caller checks it. */
  private failure = '';
  /** Whether the operating system has confirmed termination. */
  private exited = false;
  /** Installs listeners before any asynchronous work can lose a spawn error. */
  constructor(child: ChildProcess) {
    this.child = child;
    child.once('error', (error) => this.ended(`Could not launch browser: ${error.message}`));
    child.once('exit', (code) =>
      this.ended(`Browser exited before import completed (${code ?? 'signal'}). Close it and retry.`),
    );
  }
  /** Remember termination without overwriting a more useful launch error. */
  private ended(message: string): void {
    this.exited = true;
    this.failure ||= message;
  }
  /** Polls readiness while checking launch failures on every iteration. */
  async ready(scratch: string): Promise<number> {
    const deadline = Date.now() + LAUNCH_READY_TIMEOUT_MS;
    while (Date.now() < deadline) {
      if (this.failure) throw new Error(this.failure);
      const port = portAt(scratch);
      if (port) return port;
      await delay(LAUNCH_POLL_MS);
    }
    throw new Error('Browser did not become ready for import. Close the source browser and retry.');
  }
  /** Stops the process before deleting a profile that it could still be writing. */
  async clean(scratch: string): Promise<void> {
    if (!this.exited) this.child.kill();
    const deadline = Date.now() + EXIT_TIMEOUT_MS;
    while (!this.exited && Date.now() < deadline) await delay(LAUNCH_POLL_MS);
    if (!this.exited) throw new Error('Import browser did not exit. Close it before retrying import.');
    await removeCapture(scratch);
  }
}

/** Bounded asynchronous removal handles Windows releasing profile files late. */
async function removeCapture(scratch: string): Promise<void> {
  await fs.promises.rm(scratch, {
    recursive: true,
    force: true,
    maxRetries: CLEANUP_RETRIES,
    retryDelay: CLEANUP_RETRY_MS,
  });
}

/** A bounded pause between readiness and exit checks. */
function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
