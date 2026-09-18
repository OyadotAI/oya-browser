/**
 * The toast list: showing one, which then dismisses itself after a while, and
 * dismissing one early.
 */
import { useCallback, useState, type Dispatch, type SetStateAction } from 'react';
import { TOAST_LIFETIME_MS } from './constants';
import type { ShowToast, ToastItem, ToastType } from './types';

/** Ids are per page load; toasts from different providers never share one. */
let nextToastId = 0;

/** The toasts on screen, with `toast` to show one and `dismiss` to close one. */
export function useToasts() {
  const [toasts, setToasts] = useState<ToastItem[]>([]);
  const dismiss = useCallback((id: number) => setToasts((prev) => prev.filter((t) => t.id !== id)), []);
  const toast: ShowToast = useCallback(
    (msg: string, type: ToastType = 'info') => show(setToasts, dismiss, { id: nextToastId++, msg, type }),
    [dismiss],
  );
  return { toasts, toast, dismiss };
}

/** Adds a toast and schedules its dismissal. */
function show(setToasts: Dispatch<SetStateAction<ToastItem[]>>, dismiss: (id: number) => void, item: ToastItem) {
  setToasts((prev) => [...prev, item]);
  setTimeout(() => dismiss(item.id), TOAST_LIFETIME_MS);
}
