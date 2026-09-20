/**
 * Fixed values for the snippets dialog.
 */

/** The languages SyntaxCode highlights. */
export type SnippetLanguage = 'typescript' | 'json' | 'bash';

/** What the dialog shows in place of the key until the user asks to see it. */
export const MASK = '<your-api-key>';

/** How long "Copied" stays on the copy button. */
export const COPIED_RESET_MS = 1500;

/** Highlighting language per snippet id; anything not listed is TypeScript. */
export const SNIPPET_LANGUAGE: Record<string, SnippetLanguage> = { mcp: 'json', cli: 'bash', curl: 'bash' };

/** The language for snippets not in SNIPPET_LANGUAGE. */
export const DEFAULT_LANGUAGE: SnippetLanguage = 'typescript';
