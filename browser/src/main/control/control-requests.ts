/**
 * desktop_control requests to the server that are waiting for their result:
 * each gets an id, a timeout, and fails at once when the socket is down.
 */
import { randomUUID } from 'node:crypto';
import { CONTROL_REQUEST_TIMEOUT_MS } from './constants.ts';

/** One desktop_control request, as it goes over the control socket. */
export interface ControlMessage {
  /** Always 'desktop_control'. */
  type: 'desktop_control';
  /** Matches the result to the request. */
  id: string;
  /** 'request', 'acquire', 'return', 'renew', 'command-start' or 'command-end'. */
  action: string;
  /** The command slot a 'command-end' releases. */
  token?: string;
}

/** Writes to the control socket; false when it is down. */
export type SendControl = (message: ControlMessage) => boolean;

/** A request waiting on the server. */
export interface PendingRequest {
  /** Settles the request with the server's answer. */
  resolve: (value: unknown) => void;
  /** Fails the request. */
  reject: (error: Error) => void;
  /** Its timeout. */
  timer: NodeJS.Timeout;
}

/** desktop_control requests to the server that are waiting for their result. */
export class ControlRequests {
  /** Pending requests by id. */
  private readonly pending = new Map<string, PendingRequest>();
  /** Writes to the control socket. */
  private readonly send: SendControl;

  /** `send` writes to the control socket and returns false when it is down. */
  constructor(send: SendControl) {
    this.send = send;
  }

  /** Sends one desktop_control request and waits for its result. */
  request<T>(action: string, extra: Pick<ControlMessage, 'token'> = {}): Promise<T> {
    const id = randomUUID();
    return new Promise<T>((resolve, reject) => {
      const timer = setTimeout(() => this.expire(id, reject), CONTROL_REQUEST_TIMEOUT_MS);
      this.pending.set(id, { resolve: resolve as (value: unknown) => void, reject, timer });
      if (!this.send({ type: 'desktop_control', id, action, ...extra })) this.unsent(id, timer, reject);
    });
  }

  /** No answer in time. */
  private expire(id: string, reject: (error: Error) => void): void {
    this.pending.delete(id);
    reject(new Error('Control request timed out. Check your connection and retry.'));
  }

  /** The request never left: the socket is down. */
  private unsent(id: string, timer: NodeJS.Timeout, reject: (error: Error) => void): void {
    clearTimeout(timer);
    this.pending.delete(id);
    reject(new Error('Browser is disconnected'));
  }

  /** Removes and returns the request `id` answers, or undefined when none waits. */
  take(id: string): PendingRequest | undefined {
    const pending = this.pending.get(id);
    if (!pending) return undefined;
    clearTimeout(pending.timer);
    this.pending.delete(id);
    return pending;
  }

  /** Rejects every request still waiting on the server. */
  failAll(): void {
    for (const pending of this.pending.values()) {
      clearTimeout(pending.timer);
      pending.reject(new Error('Browser disconnected during handoff'));
    }
    this.pending.clear();
  }
}
