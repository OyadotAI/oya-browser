/**
 * The toast shapes shared by the provider, its state hook and the card.
 */

/** How a toast reads: a success, a failure, or a note. */
export type ToastType = 'success' | 'error' | 'info';

/** One toast on screen. */
export interface ToastItem {
  /** Unique per page load, so a toast can be dismissed by id. */
  id: number;
  /** What it says. */
  msg: string;
  /** How it reads. */
  type: ToastType;
}

/** Shows a toast. */
export type ShowToast = (msg: string, type?: ToastType) => void;
