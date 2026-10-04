/**
 * The Routines feature's entry: the pane the composition root mounts in the
 * panel, and the ViewModel it builds from the bridge and the panel.
 */
export { RoutinesPane, type RoutinesPaneProps } from './components/routines-pane.tsx';
export {
  RoutinesViewModel,
  type RoutinesServices,
  type RoutinesState,
  type RoutinesPanel,
} from './view-models/routines-view-model.ts';
export type { Routine, RoutineRun, RoutinesSnapshot, Schedule } from './model/types.ts';
