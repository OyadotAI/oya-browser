/**
 * The control feature's entry: the control bar (who drives the browser, Take
 * control), its ViewModel, and `blockedActions`, which the root reads to
 * refuse recording while the page is watch-only.
 */
export { ControlBar } from './components/control-bar.tsx';
export { ControlViewModel, blockedActions } from './view-models/control-view-model.ts';
