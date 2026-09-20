/**
 * Modal dialogs: remembers what had focus, closes on Escape, and keeps Tab
 * inside the open dialog.
 */
/* exported Dialogs */

/** The open dialog, if any. */
const Dialogs = {
  /** Focus before the dialog opened, restored when it closes. */
  lastFocus: undefined,
  /** The open dialog element. */
  active: null,
  /** Closes the open dialog. */
  close: null,

  /** A dialog opened: Escape calls `close`. */
  activate(element, close) {
    Dialogs.lastFocus = document.activeElement;
    Dialogs.active = element;
    Dialogs.close = close;
  },

  /** The dialog closed: focus goes back where it was. */
  deactivate() {
    Dialogs.active = null;
    Dialogs.close = null;
    if (Dialogs.lastFocus?.isConnected) Dialogs.lastFocus.focus();
  },

  /** Escape closes; Tab and Shift+Tab wrap around the dialog's controls. */
  keydown(event) {
    if (!Dialogs.active) return;
    if (event.key === 'Escape') {
      event.preventDefault();
      Dialogs.close?.();
      return;
    }
    if (event.key === 'Tab') Dialogs.trapTab(event);
  },

  /** Keeps Tab focus within the dialog's visible controls. */
  trapTab(event) {
    const items = Dialogs.focusable();
    const [first, last] = [items[0], items.at(-1)];
    const wrapTo = event.shiftKey ? document.activeElement === first && last : document.activeElement === last && first;
    if (wrapTo === false) return;
    event.preventDefault();
    wrapTo?.focus();
  },

  /** The dialog's visible, focusable controls, in tab order. */
  focusable() {
    const selector = 'button:not(:disabled), input, select, textarea, [tabindex="0"]';
    return [...Dialogs.active.querySelectorAll(selector)].filter((el) => el.getClientRects().length);
  },
};

document.addEventListener('keydown', Dialogs.keydown);
