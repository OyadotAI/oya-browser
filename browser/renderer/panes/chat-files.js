/**
 * The Ask pane's attachments: files picked with the paperclip wait as chips
 * above the input, go with the next message, and are sent again with every
 * later turn as the chat's data, so the agent can still upload them into a
 * page (upload_file). The agent is told each file's name, type and size; it
 * does not read what is inside.
 */
/* global Dom, RendererConstants */
/* exported ChatFiles */

/** The words the attachments show. */
const CHAT_FILES_TEXT = {
  tooBig: 'Attachments can be up to 10 MB in all.',
  unreadable: 'Could not read that file.',
  remove: 'Remove',
  attached: 'Attached',
};

/** The attachments. */
const ChatFiles = {
  /** Files picked for the next message: `{ file, type, b64 }`. */
  pending: [],
  /** Files sent earlier in this conversation. */
  sent: [],

  /** A picked File as base64, without the data: prefix. */
  read(file) {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(String(reader.result).split(',')[1] || '');
      reader.onerror = () => reject(reader.error);
      reader.readAsDataURL(file);
    });
  },

  /** Adds picked files; one that would pass the size cap, or cannot be read, stops with a note. */
  async add(files) {
    for (const file of files) {
      const b64 = await ChatFiles.read(file).catch(() => null);
      if (b64 === null) return ChatFiles.render(CHAT_FILES_TEXT.unreadable);
      if (ChatFiles.size() + b64.length > RendererConstants.CHAT_FILES_MAX_B64)
        return ChatFiles.render(CHAT_FILES_TEXT.tooBig);
      ChatFiles.pending.push({ file: file.name, type: file.type || 'application/octet-stream', b64 });
    }
    ChatFiles.render();
  },

  /** Base64 characters attached so far, sent and pending. */
  size() {
    return [...ChatFiles.sent, ...ChatFiles.pending].reduce((total, f) => total + f.b64.length, 0);
  },

  /** Drops the pending file at `index`. */
  remove(index) {
    ChatFiles.pending.splice(index, 1);
    ChatFiles.render();
  },

  /** Shows the pending files as chips, with an optional note; hidden when there is neither. */
  render(note) {
    const box = Dom.byId('chat-files');
    const chips = ChatFiles.pending.map(ChatFiles.chip);
    box.replaceChildren(...chips, ...(note ? [Dom.node('span', note, 'chat-files-note')] : []));
    box.hidden = !chips.length && !note;
  },

  /** One pending file's chip, with a button that removes it. */
  chip(f, index) {
    const chip = Dom.node('span', null, 'chat-file');
    chip.append(Dom.node('span', f.file, 'chat-file-name'));
    const remove = Dom.node('button', '×', 'chat-file-remove');
    remove.setAttribute('aria-label', `${CHAT_FILES_TEXT.remove} ${f.file}`);
    remove.addEventListener('click', () => ChatFiles.remove(index));
    chip.append(remove);
    return chip;
  },

  /** `text` as the message to send, naming the files that go with it; they count as sent from here on. */
  attachTo(text) {
    const names = ChatFiles.pending.map((f) => f.file);
    ChatFiles.sent.push(...ChatFiles.pending.splice(0));
    ChatFiles.render();
    return names.length ? `${text}\n\n(${CHAT_FILES_TEXT.attached}: ${names.join(', ')})` : text;
  },

  /** Every file of the conversation, as the chat route's `data` (`file1`, `file2`, …); undefined when there are none. */
  data() {
    if (!ChatFiles.sent.length) return undefined;
    return Object.fromEntries(ChatFiles.sent.map((f, i) => [`file${i + 1}`, f]));
  },

  /** Forgets every file (Clear). */
  clear() {
    ChatFiles.pending = [];
    ChatFiles.sent = [];
    ChatFiles.render();
  },

  /** Opens the file picker. */
  pick() {
    Dom.byId('chat-file-input').click();
  },

  /** Takes what the picker chose, and empties it so the same file can be picked again. */
  picked(e) {
    const input = e.target;
    ChatFiles.add([...(input.files || [])]);
    input.value = '';
  },
};

Dom.byId('chat-attach').addEventListener('click', ChatFiles.pick);
Dom.byId('chat-file-input').addEventListener('change', ChatFiles.picked);
