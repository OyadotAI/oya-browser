/**
 * Binds a React view to a ViewModel: the view re-renders when the ViewModel's
 * state is replaced, and reads it as a plain snapshot.
 */
import { useSyncExternalStore } from 'react';
import type { ViewModel } from '../core/view-model.ts';

/** The ViewModel's current state, kept current. */
export function useViewModel<S extends object>(vm: ViewModel<S>): S {
  return useSyncExternalStore(vm.subscribe, vm.getSnapshot);
}
