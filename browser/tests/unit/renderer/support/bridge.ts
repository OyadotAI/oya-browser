/**
 * A fake window.oyaBrowser for ViewModel tests: every call is recorded and
 * answered from `answers` (a value, or a function of the arguments), and a test
 * fires an event with `emit('onTabsUpdated', payload)`.
 */
import { CALL_CHANNELS, EVENT_CHANNELS, type OyaBrowser } from '../../../../src/shared/ipc.ts';

/** A fake bridge and the handles a test drives it with. */
export interface FakeBridge {
  /** The bridge to hand a ViewModel. */
  bridge: OyaBrowser;
  /** Every call made, as [method, ...args]. */
  calls: unknown[][];
  /** The calls made to `method`, as their argument lists. */
  called(method: string): unknown[][];
  /** Fires the event `name` (e.g. 'onTabsUpdated') with `payload`. */
  emit(name: string, payload?: unknown): void;
  /** How many listeners `name` has (0 once a ViewModel disposed). */
  listeners(name: string): number;
}

/** A fake bridge answering calls from `answers`. */
export function fakeBridge(answers: Record<string, unknown> = {}): FakeBridge {
  const calls: unknown[][] = [];
  const listeners = new Map<string, Set<(payload: unknown) => void>>();
  const api: Record<string, unknown> = {};
  for (const name of Object.keys(CALL_CHANNELS)) {
    api[name] = async (...args: unknown[]) => {
      calls.push([name, ...args]);
      const answer = answers[name];
      return typeof answer === 'function' ? answer(...args) : answer;
    };
  }
  for (const name of Object.keys(EVENT_CHANNELS)) {
    api[name] = (listener: (payload: unknown) => void) => {
      const set = listeners.get(name) ?? new Set();
      listeners.set(name, set.add(listener));
      return () => void set.delete(listener);
    };
  }
  return {
    bridge: api as unknown as OyaBrowser,
    calls,
    called: (method) => calls.filter(([name]) => name === method).map(([, ...args]) => args),
    emit: (name, payload) => listeners.get(name)?.forEach((listener) => listener(payload)),
    listeners: (name) => listeners.get(name)?.size ?? 0,
  };
}

/** A frame clock a test advances by hand. */
export function manualFrames() {
  const pending = new Map<number, () => void>();
  let next = 1;
  return {
    request: (callback: () => void) => (pending.set(next, callback), next++),
    cancel: (handle: number) => void pending.delete(handle),
    /** Runs the frames requested so far. */
    run() {
      const due = [...pending.values()];
      pending.clear();
      due.forEach((callback) => callback());
    },
    /** How many frames wait to run. */
    get waiting() {
      return pending.size;
    },
  };
}
