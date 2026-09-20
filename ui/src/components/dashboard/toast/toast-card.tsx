/**
 * One toast: an icon for its type, the message, and a close button.
 */
'use client';

import { motion } from 'framer-motion';
import { X, CheckCircle2, AlertCircle, Info } from 'lucide-react';
import { TOAST_HIDDEN, TOAST_SHOWN, TOAST_TRANSITION } from './constants';
import type { ToastItem, ToastType } from './types';

/** Colors per type. */
const TONE: Record<ToastType, string> = {
  success: 'bg-accent/10 border-accent/25 text-accent',
  error: 'bg-red-500/10 border-red-500/20 text-red-400',
  info: 'bg-indigo-500/10 border-indigo-500/20 text-indigo-400',
};

/** Icon per type. */
const ICON: Record<ToastType, typeof Info> = { success: CheckCircle2, error: AlertCircle, info: Info };

/** What a card shows and how it closes. */
interface Props {
  /** The toast. */
  item: ToastItem;
  /** Closes it early. */
  onDismiss: () => void;
}

/** A toast that animates in and out. */
export default function ToastCard({ item, onDismiss }: Props) {
  const Icon = ICON[item.type];
  return (
    <motion.div
      initial={TOAST_HIDDEN}
      animate={TOAST_SHOWN}
      exit={TOAST_HIDDEN}
      transition={TOAST_TRANSITION}
      className={`pointer-events-auto flex items-center gap-2 px-4 py-2.5 rounded-lg text-sm font-medium shadow-xl shadow-black/40 border backdrop-blur-sm ${TONE[item.type]}`}
    >
      <Icon className="w-4 h-4 shrink-0" />
      <span>{item.msg}</span>
      <button onClick={onDismiss} className="ml-1 opacity-60 hover:opacity-100 transition-colors">
        <X className="w-3.5 h-3.5" />
      </button>
    </motion.div>
  );
}
