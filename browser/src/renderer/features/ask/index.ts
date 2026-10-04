/**
 * The Ask feature's entry: the pane the composition root mounts in the
 * panel, and the ViewModel it builds (whose `ask(text)` the start page calls,
 * and whose `run` state drives the panel header's orb).
 */
export { AskPane, type AskPaneProps } from './components/ask-pane.tsx';
export { AskViewModel, type AskServices, type AskState, type ChatItem } from './view-models/ask-view-model.ts';
