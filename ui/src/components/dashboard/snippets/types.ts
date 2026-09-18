/**
 * The shape of one copyable code sample in the snippets dialog.
 */

/** One tab of the snippets dialog: a language, its file name and the code for a key. */
export interface Snippet {
  /** Stable tab id; also picks the highlighting language. */
  id: string;
  /** Tab label. */
  label: string;
  /** File name shown above the code. */
  file: string;
  /** The code with `key` filled in (the real key, or the mask). */
  code: (key: string) => string;
  /** A line under the code for what the sample does not show. */
  note?: string;
}
