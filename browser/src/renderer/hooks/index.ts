/**
 * The shell page's shared hooks, used by more than one feature: binding a
 * view to its ViewModel, a modal dialog's focus, closing a popover, keeping a
 * list at its newest row, roving focus along tabs, and reduced motion.
 */
export { useViewModel } from './use-view-model.ts';
export { useDialog } from './use-dialog.ts';
export { useClickOutside, useEscapeKey } from './use-dismiss.ts';
export { useStickToBottom } from './use-stick-to-bottom.ts';
export { useRovingFocus, rovingIndex, isRovingKey } from './use-roving-focus.ts';
export { reducedMotion } from './reduced-motion.ts';
export { ROVING_KEYS, type RovingKey } from './constants.ts';
