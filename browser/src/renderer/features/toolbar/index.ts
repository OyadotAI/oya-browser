/**
 * The navigation toolbar's entry: the toolbar the composition root mounts
 * (with the other features' toolbar parts as children), and its ViewModel,
 * whose `focusAddress()` satisfies the command palette's host.
 */
export { Toolbar, type ToolbarProps } from './components/toolbar.tsx';
export { ToolbarViewModel, type ToolbarState, type ToolbarBridge } from './view-models/toolbar-view-model.ts';
