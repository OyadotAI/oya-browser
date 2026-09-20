/**
 * Toasts for the dashboard: `ToastProvider` shows them in the top-right
 * corner and `useToast` gives any component inside it a way to raise one.
 */
'use client';

import { createContext, useContext, useMemo, type ReactNode } from 'react';
import { AnimatePresence } from 'framer-motion';
import ToastCard from './toast/toast-card';
import { useToasts } from './toast/use-toasts';
import type { ShowToast } from './toast/types';

export type { ToastItem } from './toast/types';

/** What the provider hands down. */
interface ToastContextType {
  /** Shows a toast. */
  toast: ShowToast;
}

/** Null outside a provider, which `useToast` turns into a clear error. */
const ToastContext = createContext<ToastContextType | null>(null);

/** What the provider wraps. */
interface ProviderProps {
  /** The dashboard. */
  children: ReactNode;
}

/** Holds the toasts and renders them above everything else. */
export function ToastProvider({ children }: ProviderProps) {
  const { toasts, toast, dismiss } = useToasts();
  const value = useMemo(() => ({ toast }), [toast]);
  return (
    <ToastContext.Provider value={value}>
      {children}
      <div className="fixed top-4 right-4 z-[9999] flex flex-col gap-2 pointer-events-none">
        <AnimatePresence mode="popLayout">
          {toasts.map((t) => (
            <ToastCard key={t.id} item={t} onDismiss={() => dismiss(t.id)} />
          ))}
        </AnimatePresence>
      </div>
    </ToastContext.Provider>
  );
}

/** The function that raises a toast. Throws outside a ToastProvider. */
export function useToast() {
  const ctx = useContext(ToastContext);
  if (!ctx) throw new Error('useToast must be used within ToastProvider');
  return ctx.toast;
}
