/**
 * A dialog that fired mid-action is reported on the next tool result rather
 * than lost. Both transports record it here.
 */

/** browserId → the dialog note waiting to be reported. */
const notes = new Map<string, string>();

/** Keeps the dialog a command result mentions, if any. */
export function noteDialog(browserId: string, result) {
  if (result?.data?.dialog) notes.set(browserId, result.data.dialog);
}

/** The dialog note waiting for this browser, cleared once taken. */
export function takeDialogNote(browserId: string) {
  const note = notes.get(browserId);
  if (note) notes.delete(browserId);
  return note || null;
}
