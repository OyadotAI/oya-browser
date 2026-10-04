/**
 * The workflow studio's entry: the Record pane the composition root mounts in
 * the panel, its ViewModel, and the `RecordingGate` the root satisfies with
 * the control bar.
 */
export { RecordPane, type RecordPaneProps } from './components/record-pane.tsx';
export { StudioViewModel } from './view-models/studio-view-model.ts';
export type { RecordingGate } from './view-models/studio-actions.ts';
