/**
 * A CodeMirror 6 editor as a React component: line numbers, folding, bracket
 * matching, search (⌘F), undo history, and for JSON a linter that marks a parse
 * error where it is. The text stays the parent's: a change goes up through
 * `onChange`, and a new `value` from outside (a Variables edit, Discard) is put in
 * without losing the cursor's line.
 */
'use client';

import { useEffect, useRef } from 'react';
import { basicSetup } from 'codemirror';
import { EditorState, type Extension } from '@codemirror/state';
import { EditorView, keymap } from '@codemirror/view';
import { json, jsonParseLinter } from '@codemirror/lang-json';
import { javascript } from '@codemirror/lang-javascript';
import { linter, lintGutter } from '@codemirror/lint';
import { editorLook } from './cm-theme';

/** Props for the editor. */
export interface CodeEditorProps {
  /** The text. */
  value: string;
  /** json gets a linter; typescript is shown as code. */
  language: 'json' | 'typescript';
  /** Names the editor for assistive tech (and tests). */
  label: string;
  /** Shown only: no typing. */
  readOnly?: boolean;
  /** The text changed. */
  onChange?: (text: string) => void;
  /** The cursor moved: its line and column, from 1. */
  onCursor?: (line: number, col: number) => void;
  /** ⌘S / Ctrl+S inside the editor. */
  onSave?: () => void;
}

/** The latest props, read by the editor's callbacks without rebuilding it. */
interface Latest {
  /** The props as last rendered. */
  current: CodeEditorProps;
}

/** The language support, with JSON's linter. */
const languageOf = (language: CodeEditorProps['language']): Extension[] =>
  language === 'json' ? [json(), linter(jsonParseLinter()), lintGutter()] : [javascript({ typescript: true })];

/** Reports changes and cursor moves to the latest callbacks. */
function listener(props: Latest): Extension {
  return EditorView.updateListener.of((u) => {
    if (u.docChanged) props.current.onChange?.(u.state.doc.toString());
    if (u.selectionSet || u.docChanged) {
      const head = u.state.selection.main.head;
      const line = u.state.doc.lineAt(head);
      props.current.onCursor?.(line.number, head - line.from + 1);
    }
  });
}

/** Everything the editor is built with. */
function extensionsOf(props: Latest): Extension[] {
  const { language, label, readOnly } = props.current;
  const save = keymap.of([{ key: 'Mod-s', preventDefault: true, run: () => (props.current.onSave?.(), true) }]);
  const attrs = EditorView.contentAttributes.of({ 'aria-label': label });
  const lock = [EditorState.readOnly.of(Boolean(readOnly)), EditorView.editable.of(!readOnly)];
  return [save, basicSetup, ...languageOf(language), ...editorLook, attrs, ...lock, listener(props)];
}

/** The editor, made once per language; outside changes to `value` are put in place. */
export default function CodeEditor(props: CodeEditorProps) {
  const host = useRef<HTMLDivElement>(null);
  const view = useRef<EditorView | null>(null);
  const latest = useRef(props);
  useEffect(() => void (latest.current = props));
  useEffect(() => {
    const state = EditorState.create({ doc: latest.current.value, extensions: extensionsOf(latest) });
    view.current = new EditorView({ state, parent: host.current! });
    return () => view.current?.destroy();
  }, [props.language, props.readOnly]);
  useEffect(() => {
    const v = view.current;
    if (v && v.state.doc.toString() !== props.value)
      v.dispatch({ changes: { from: 0, to: v.state.doc.length, insert: props.value } });
  }, [props.value]);
  return <div ref={host} className="h-full min-h-0 overflow-hidden" />;
}
