/**
 * Everything the project switcher renders from: the session and the
 * popover's state as one PickerContext, plus the DOM refs for focus handling.
 */
import { useCallback, useEffect, useRef, useState, type RefObject } from 'react';
import { useAuth } from '@/components/auth-provider';
import { useToast } from '../toast';
import { useOutsideClick } from '../hooks/use-outside-click';
import { INITIAL_PICKER } from './constants';
import type { PickerState, Session } from './types';
import { useProjectSession } from './use-project-session';

/** Changes merged into the popover's state. */
type Patch = (changes: Partial<PickerState>) => void;

/** The popover's state, and a stable way to merge changes into it. */
function usePickerState() {
  const [ui, setUi] = useState<PickerState>(INITIAL_PICKER);
  const patch = useCallback((changes: Partial<PickerState>) => setUi((s) => ({ ...s, ...changes })), []);
  return { ui, patch };
}

/** A click outside closes the popover and drops any key or secret on screen; opening "manage" focuses its first action. */
function usePopoverEffects(rootRef: RefObject<HTMLDivElement | null>, ui: PickerState, patch: Patch) {
  const dismiss = useCallback(() => patch({ open: false, revealedKey: '', secret: '' }), [patch]);
  useOutsideClick(rootRef, dismiss);
  useEffect(() => {
    if (ui.open && ui.form === 'manage')
      rootRef.current?.querySelector<HTMLButtonElement>('[data-management-action]')?.focus();
  }, [rootRef, ui.open, ui.form]);
}

/** The popover's state, its effects, and the refs for its root and button. */
function usePicker() {
  const { ui, patch } = usePickerState();
  const rootRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  usePopoverEffects(rootRef, ui, patch);
  const focusTrigger = useCallback(() => triggerRef.current?.focus(), []);
  return { ui, patch, focusTrigger, rootRef, triggerRef };
}

/** The switcher's context for this render (`c`), and the refs for its root and button. */
export function useProjectSwitcher(setApiKey: Session['setApiKey']) {
  const { token } = useAuth();
  const toast = useToast();
  const { rootRef, triggerRef, ...picker } = usePicker();
  return { c: { ...useProjectSession(token, setApiKey, toast), ...picker }, rootRef, triggerRef };
}
