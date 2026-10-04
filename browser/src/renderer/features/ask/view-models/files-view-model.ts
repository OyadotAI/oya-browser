/**
 * The Ask pane's attachments: files picked with the paperclip wait as chips
 * above the input, go with the next message, and are sent again with every
 * later turn as the chat's data, so the agent can still upload them into a
 * page (upload_file). The agent is told each file's name, type and size; it
 * does not read what is inside.
 */
import { ViewModel } from '../../../core/view-model.ts';
import { RendererConstants as C } from '../../../core/constants.ts';
import { ASK_TEXT } from '../model/constants.ts';

/** One attached file, as the chat route takes it. */
export interface AttachedFile {
  /** Its name. */
  file: string;
  /** Its media type. */
  type: string;
  /** Its bytes, base64, without the data: prefix. */
  b64: string;
}

/** What the attachments show. */
export interface FilesState {
  /** Files picked for the next message. */
  pending: AttachedFile[];
  /** Why the last pick stopped, or ''. */
  note: string;
}

/** Reads a picked file as base64. */
export type ReadFile = (file: File) => Promise<string>;

/** The media type of a file that does not say. */
const UNKNOWN_TYPE = 'application/octet-stream';

/** A picked file as base64, without the data: prefix. */
export function readBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result).split(',')[1] || '');
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });
}

/** The attachments. */
export class FilesViewModel extends ViewModel<FilesState> {
  /** Files sent earlier in this conversation. */
  private sent: AttachedFile[] = [];
  /** How a picked file is read. */
  private readonly read: ReadFile;

  /** Empty; `read` reads a picked file (FileReader in the page, a fake in tests). */
  constructor(read: ReadFile = readBase64) {
    super({ pending: [], note: '' });
    this.read = read;
  }

  /** Adds picked files; one that would pass the size cap, or cannot be read, stops with a note. */
  async add(files: readonly File[]): Promise<void> {
    const pending = [...this.state.pending];
    for (const file of files) {
      const b64 = await this.read(file).catch(() => null);
      const note = this.refusal(b64, pending);
      if (b64 === null || note) return this.set({ pending, note });
      pending.push({ file: file.name, type: file.type || UNKNOWN_TYPE, b64 });
    }
    this.set({ pending, note: '' });
  }

  /** Drops the pending file at `index`. */
  remove(index: number): void {
    this.set({ pending: this.state.pending.filter((_, i) => i !== index), note: '' });
  }

  /** `text` as the message to send, naming the files that go with it; they count as sent from here on. */
  attachTo(text: string): string {
    const names = this.state.pending.map((f) => f.file);
    this.sent.push(...this.state.pending);
    this.set({ pending: [], note: '' });
    return names.length ? `${text}\n\n(${ASK_TEXT.attached}: ${names.join(', ')})` : text;
  }

  /** Every file of the conversation, as the chat route's `data` (`file1`, `file2`, …); undefined when there are none. */
  data(): Record<string, AttachedFile> | undefined {
    if (!this.sent.length) return undefined;
    return Object.fromEntries(this.sent.map((f, i) => [`file${i + 1}`, f]));
  }

  /** Forgets every file (Clear). */
  clear(): void {
    this.sent = [];
    this.set({ pending: [], note: '' });
  }

  /** Why a file read as `b64` cannot join `pending` (unreadable, or past the cap), or '' when it can. */
  private refusal(b64: string | null, pending: readonly AttachedFile[]): string {
    if (b64 === null) return ASK_TEXT.unreadable;
    return this.size(pending) + b64.length > C.CHAT_FILES_MAX_B64 ? ASK_TEXT.tooBig : '';
  }

  /** Base64 characters attached so far, sent and `pending`. */
  private size(pending: readonly AttachedFile[]): number {
    return [...this.sent, ...pending].reduce((total, f) => total + f.b64.length, 0);
  }
}
