/**
 * The Ask pane's input bar: the attached files as chips, the paperclip, the
 * Ask box (Enter sends, Shift+Enter and IME composition insert text; it grows
 * with its text up to a limit), and Send, with Stop in its place while the
 * agent works. The box takes focus when the panel opens on Ask and after an
 * answer.
 */
import { useEffect, useLayoutEffect, useRef, type KeyboardEvent } from 'react';
import { useViewModel } from '../../../hooks/index.ts';
import { IconButton } from '../../../ui/index.ts';
import { RendererConstants as C } from '../../../core/constants.ts';
import type { RendererServices } from '../../../app/services.ts';
import type { AskViewModel } from '../view-models/ask-view-model.ts';
import type { FilesViewModel } from '../view-models/files-view-model.ts';

/** What the input bar is given. */
interface ComposerProps {
  /** The conversation. */
  ask: AskViewModel;
  /** The panel, to focus the box when it opens on Ask. */
  panel: RendererServices['panel'];
}

/** The pending files as chips, each with a remove button, and a note when a pick stopped. */
function FileChips({ files }: { /** The attachments. */ files: FilesViewModel }) {
  const { pending, note } = useViewModel(files);
  return (
    <div className="chat-files" id="chat-files" aria-label="Attached files" hidden={!pending.length && !note}>
      {pending.map((f, i) => (
        <span key={i} className="chat-file">
          <span className="chat-file-name">{f.file}</span>
          <button className="chat-file-remove" aria-label={`Remove ${f.file}`} onClick={() => files.remove(i)}>
            ×
          </button>
        </span>
      ))}
      {note && <span className="chat-files-note">{note}</span>}
    </div>
  );
}

/**
 * Fits the box to its text, up to a limit. An empty box keeps the height its
 * stylesheet gives it: measured while the pane is hidden, it would collapse.
 */
function fitText(el: HTMLTextAreaElement, text: string): void {
  el.style.height = 'auto';
  if (text) el.style.height = `${Math.min(el.scrollHeight, C.CHAT_INPUT_MAX_PX)}px`;
  else el.style.height = '';
}

/** The Ask box: fits its text up to a limit, and takes focus when the panel opens on Ask and once an answer is in. */
function useAskBox({ ask, panel }: ComposerProps) {
  const { input, sending } = useViewModel(ask);
  const { open, pane } = useViewModel(panel);
  const box = useRef<HTMLTextAreaElement>(null);
  useLayoutEffect(() => void (box.current && fitText(box.current, input)), [input]);
  useEffect(() => void (open && pane === 'chat' && !sending && box.current?.focus()), [open, pane, sending]);
  return box;
}

/** Enter sends; Shift+Enter and IME composition insert text as usual. */
const isSendKey = (e: KeyboardEvent) => e.key === 'Enter' && !e.shiftKey && !e.nativeEvent.isComposing;

/** What the composer's buttons are given. */
interface AskProps {
  /** The conversation. */
  ask: AskViewModel;
}

/** The paperclip and the hidden file input it opens; the input is emptied so the same file can be picked again. */
function AttachButton({ ask }: AskProps) {
  const picker = useRef<HTMLInputElement>(null);
  return (
    <>
      <input
        type="file"
        id="chat-file-input"
        multiple
        hidden
        ref={picker}
        onChange={(e) => (void ask.files.add([...(e.target.files ?? [])]), (e.target.value = ''))}
      />
      <IconButton
        className="icon-button chat-attach"
        id="chat-attach"
        aria-label="Attach files"
        title="Attach files the agent can upload to a page"
        data-icon="attach"
        onClick={() => picker.current?.click()}
        icon="attach"
      />
    </>
  );
}

/** Send, with Stop in its place while the agent works. */
function SendButtons({ ask }: AskProps) {
  const { sending } = useViewModel(ask);
  return (
    <>
      <IconButton
        className="chat-send"
        id="chat-send"
        aria-label="Send message"
        data-icon="send"
        hidden={sending}
        onClick={() => void ask.send()}
        icon="send"
      />
      <IconButton
        className="chat-send chat-stop"
        id="chat-stop"
        label="Stop"
        data-icon="stop"
        hidden={!sending}
        onClick={() => ask.stop()}
        icon="stop"
      />
    </>
  );
}

/** The input bar. */
export function Composer({ ask, panel }: ComposerProps) {
  const { input } = useViewModel(ask);
  const box = useAskBox({ ask, panel });
  return (
    <div className="chat-input-bar">
      <FileChips files={ask.files} />
      <div className="chat-composer">
        <AttachButton ask={ask} />
        <textarea
          className="chat-input"
          id="chat-input"
          ref={box}
          rows={1}
          aria-label="Ask Oya"
          placeholder="Ask about this page or give a task…"
          spellCheck={false}
          value={input}
          onChange={(e) => ask.setInput(e.target.value)}
          onKeyDown={(e) => isSendKey(e) && (e.preventDefault(), void ask.send())}
        />
        <SendButtons ask={ask} />
      </div>
      <p className="chat-hint">Enter to send · Shift Enter for a new line</p>
    </div>
  );
}
