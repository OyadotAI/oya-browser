/** Transport-independent dialog responses used by browser commands. */
/** What the sentence about a dialog is made of. */
export interface DialogText {
  /** alert, confirm, prompt or beforeunload. */
  type: string;
  /** The text the page showed. */
  message: string;
  /** A prompt's prefilled answer. */
  defaultPrompt?: string;
}

/** A browser-owned dialog and its one-shot reply. */
export interface Dialog extends DialogText {
  /** Identity used to cancel this exact pending dialog. */
  source: object;
  /** Native or transitional transport callback, never a protocol name. */
  reply: (accept: boolean, text?: string) => void | Promise<unknown>;
}

/** A wait for a held dialog, and how to stop waiting. */
export interface DialogWait {
  /** Settles with DIALOG_HELD when a dialog is held. */
  held: Promise<typeof import('./constants.ts').DIALOG_HELD>;
  /** Stops waiting; call it once the answer no longer matters. */
  release: () => void;
}

/** What answer() reports. */
export interface DialogAnswer {
  /** Whether a dialog was open to answer. */
  ok: boolean;
  /** Why not, when none was. */
  error?: string;
  /** What was answered. */
  data?: AnsweredDialog;
}

/** A dialog answer() closed, and how. */
export interface AnsweredDialog {
  /** The dialog's type. */
  type: string;
  /** Its text. */
  message: string;
  /** Whether it was accepted rather than dismissed. */
  accepted: boolean;
}
