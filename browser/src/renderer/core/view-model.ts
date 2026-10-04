/**
 * The ViewModel base: a ViewModel holds a view's state as one immutable
 * snapshot, changes it only through `set`, and tells its subscribers. Views
 * read it with `useViewModel`; tests read `state` directly, with no DOM.
 */

/** Called after the state changed. */
export type Listener = () => void;

/** A view's state and the intents that change it. Subclasses add the intents. */
export class ViewModel<S extends object> {
  /** The current snapshot; replaced, never mutated. */
  private snapshot: S;
  /** Who to tell when the snapshot is replaced. */
  private readonly listeners = new Set<Listener>();
  /** Undoes what the ViewModel subscribed to, on dispose. */
  private readonly cleanups: (() => void)[] = [];

  /** Starts from `initial`. */
  constructor(initial: S) {
    this.snapshot = initial;
  }

  /** The current state. */
  get state(): S {
    return this.snapshot;
  }

  /** Replaces the fields in `patch`, and tells the subscribers if anything changed. */
  protected set(patch: Partial<S>): void {
    const keys = Object.keys(patch) as (keyof S)[];
    if (keys.every((key) => Object.is(this.snapshot[key], patch[key]))) return;
    this.snapshot = { ...this.snapshot, ...patch };
    for (const listener of this.listeners) listener();
  }

  /** Keeps `cleanup` (an unsubscribe, a timer's clear) to run on dispose. */
  protected own(cleanup: () => void): void {
    this.cleanups.push(cleanup);
  }

  /** Subscribes to changes; the returned function unsubscribes. React's useSyncExternalStore calls it. */
  readonly subscribe = (listener: Listener): (() => void) => {
    this.listeners.add(listener);
    return () => void this.listeners.delete(listener);
  };

  /** The snapshot, for useSyncExternalStore. */
  readonly getSnapshot = (): S => this.snapshot;

  /** Stops everything the ViewModel subscribed to or started. */
  dispose(): void {
    for (const cleanup of this.cleanups.splice(0)) cleanup();
    this.listeners.clear();
  }
}
