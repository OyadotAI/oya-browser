/**
 * The Inspect feature's entry: the Actions, Activity and Source panes the
 * composition root mounts in the panel, and the ViewModels it builds from the
 * bridge, the panel and the clipboard, and the Source formats' labels (the
 * page-format picker in the shell dialog's footer).
 */
export { ActionsPane, type ActionsPaneProps } from './components/actions-pane.tsx';
export { NetworkPane, type NetworkPaneProps } from './components/network-pane.tsx';
export { SourcePane, type SourcePaneProps } from './components/source-pane.tsx';
export { ActionsViewModel, type ActionsState } from './view-models/actions-view-model.ts';
export { NetLogViewModel, type NetLogState } from './view-models/net-log-view-model.ts';
export { SourceViewModel, type SourceState } from './view-models/source-view-model.ts';
export { FORMAT_LABELS } from './model/constants.ts';
export type { InspectServices, InspectPanel, InspectClipboard } from './model/services.ts';
