/**
 * State held as one object and changed by merging partial updates, for views
 * with many small flags that change together.
 */
'use client';

import { useCallback, useState } from 'react';

/** A partial update, or a function of the current state that returns one. */
export type PatchUpdate<T> = Partial<T> | ((state: T) => Partial<T>);

/** Merges an update into the state; stable for the component's life. */
export type Patch<T> = (update: PatchUpdate<T>) => void;

/** The state and a stable `patch` that merges updates into it. */
export function usePatchState<T extends object>(initial: T): [T, Patch<T>] {
  const [state, setState] = useState(initial);
  const patch = useCallback<Patch<T>>(
    (update) => setState((s) => ({ ...s, ...(typeof update === 'function' ? update(s) : update) })),
    [],
  );
  return [state, patch];
}
