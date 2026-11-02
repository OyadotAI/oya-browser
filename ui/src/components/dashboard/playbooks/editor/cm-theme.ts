/**
 * How the playbook editor's CodeMirror looks: the console's own colours, so it
 * follows light and dark, a VS Code-like gutter and active line, and syntax as
 * classes (tok-*) the console's code palette colours in globals.css.
 */
import { EditorView } from '@codemirror/view';
import { syntaxHighlighting } from '@codemirror/language';
import { classHighlighter } from '@lezer/highlight';

/** The editor chrome, in the console's colour tokens. */
const chrome = EditorView.theme({
  '&': { height: '100%', color: 'var(--foreground)', backgroundColor: 'var(--background)', fontSize: '12.5px' },
  '.cm-scroller': { fontFamily: 'var(--font-mono)', lineHeight: '20px', overflow: 'auto' },
  '.cm-content': { padding: '8px 0', caretColor: 'var(--foreground)' },
  '.cm-gutters': { backgroundColor: 'var(--background)', color: 'var(--dim)', border: 'none', minWidth: '48px' },
  '.cm-activeLine, .cm-activeLineGutter': { backgroundColor: 'color-mix(in srgb, var(--foreground) 6%, transparent)' },
  '&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection': {
    backgroundColor: 'color-mix(in srgb, var(--primary) 18%, transparent) !important',
  },
  '.cm-cursor': { borderLeftColor: 'var(--foreground)' },
  '.cm-tooltip': {
    backgroundColor: 'var(--elevated)',
    color: 'var(--foreground)',
    border: '1px solid var(--border)',
    borderRadius: '6px',
  },
  '.cm-panels': { backgroundColor: 'var(--elevated)', color: 'var(--foreground)' },
  '&.cm-focused': { outline: 'none' },
});

/** The theme and the class-based highlighting together. */
export const editorLook = [chrome, syntaxHighlighting(classHighlighter)];
