/**
 * The Ask pane: a chat with the agent about the current page. Replies are
 * rendered from a small, escaped subset of Markdown: paragraphs, headings,
 * lists, code blocks, inline code, bold and italic.
 */
/* global oyaBrowser, Dom, RendererConstants, ShellIcons, ChatPlaybook, ChatProgress, ChatFiles, ChatPersona, ChatModel */
/* exported Chat */

/** A Markdown list item: `-`, `*`, `•` or `1.` / `1)` at the start of a line. */
const LIST_ITEM = /^\s*(?:[-*•]|\d+[.)])\s+/;

/** A Markdown heading line. */
const HEADING = /^#{1,6}\s+(.+)$/;

/** The words the chat shows. */
const CHAT_TEXT = {
  noResponse: '(no response)',
  stopped: 'Stopped.',
  /** The error the main process answers a stopped chat with. */
  stoppedError: 'Stopped',
  /** The codes the server refuses a chat with when the project has no model key, or the provider refused it. */
  keyCodes: ['llm_unconfigured', 'llm_rejected'],
};

/** The chat. */
const Chat = {
  /** The conversation so far, sent with each message. */
  history: [],
  /** A message is on its way. */
  sending: false,
  /** The empty state as the page first drew it, restored by Clear. */
  emptyHtml: '',
  /** Counts Clears, so an answer to a conversation already cleared is dropped. */
  epoch: 0,

  /** Inline Markdown rewrites applied, in order, to escaped text. */
  INLINE: [
    [/`([^`]+)`/g, '<code>$1</code>'],
    [/\*\*(.+?)\*\*/g, '<strong>$1</strong>'],
    [/(?<![*\w])\*([^*\n]+)\*(?![*\w])/g, '<em>$1</em>'],
  ],

  /** Escaped text with the Markdown subset turned into HTML. Code fences are split out first. */
  mdToHtml(text) {
    const parts = Dom.esc(text).split(/```[\w-]*\n?([\s\S]*?)```/);
    // split() with a capture group alternates prose and code: code sits at the odd indexes.
    return parts.map((part, i) => (i & 1 ? Chat.codeBlock(part) : Chat.prose(part))).join('');
  },

  /** One fenced code block, kept verbatim. */
  codeBlock(code) {
    return '<pre><code>' + code.replace(/\n$/, '') + '</code></pre>';
  },

  /** Text between code blocks: blank lines and headings start new blocks. */
  prose(text) {
    const blocks = text.replace(/^(#{1,6}\s.+)$/gm, '\n$1\n').split(/\n\s*\n/);
    const filled = blocks.map((block) => block.trim()).filter(Boolean);
    return filled.map(Chat.block).join('');
  },

  /** A heading, or runs of paragraph lines and list items. */
  block(block) {
    const heading = HEADING.exec(block);
    if (heading) return '<h4>' + Chat.inline(heading[1]) + '</h4>';
    return Chat.runs(block.split('\n')).map(Chat.run).join('');
  },

  /** Consecutive lines grouped by whether they are list items. */
  runs(lines) {
    const runs = [];
    for (const line of lines) {
      const last = runs.at(-1);
      if (last && LIST_ITEM.test(last[0]) === LIST_ITEM.test(line)) last.push(line);
      else runs.push([line]);
    }
    return runs;
  },

  /** A run as a list (numbered when it starts with a number) or a paragraph. */
  run(lines) {
    if (!LIST_ITEM.test(lines[0])) return '<p>' + lines.map(Chat.inline).join('<br>') + '</p>';
    const tag = /^\s*\d/.test(lines[0]) ? 'ol' : 'ul';
    const items = lines.map((line) => '<li>' + Chat.inline(line.replace(LIST_ITEM, '')) + '</li>');
    return `<${tag}>${items.join('')}</${tag}>`;
  },

  /** Inline code, bold and italic. */
  inline(text) {
    return Chat.INLINE.reduce((s, [pattern, replacement]) => s.replace(pattern, replacement), text);
  },

  /** Removes the empty-state placeholder, if shown. */
  dropEmpty() {
    Dom.byId('chat-messages').querySelector('.chat-empty')?.remove();
  },

  /** A message's markup, with badges for the tools the agent used. */
  messageHtml(role, content, toolCalls) {
    const body = role === 'user' ? Dom.esc(content) : Chat.mdToHtml(content);
    if (!toolCalls?.length) return body;
    const badges = toolCalls.map((t) => '<span class="chat-tool-badge">' + Dom.esc(t.name) + '</span>');
    return body + '<div class="chat-tools">' + badges.join('') + '</div>';
  },

  /** Appends a message (replacing the thinking indicator), scrolls to it, and answers its element. */
  addChatMessage(role, content, toolCalls) {
    const list = Dom.byId('chat-messages');
    Chat.dropEmpty();
    list.querySelector('.chat-thinking')?.remove();
    const node = list.appendChild(Chat.messageNode(role, content, toolCalls));
    list.scrollTop = list.scrollHeight;
    return node;
  },

  /** One message's element: an assistant error is marked, an assistant reply gets a Copy button. */
  messageNode(role, content, toolCalls) {
    const div = Dom.node('div', null, 'chat-msg ' + role);
    const failed = role === 'assistant' && content.startsWith('Error:');
    if (failed) div.classList.add('error');
    div.innerHTML = Chat.messageHtml(role, content, toolCalls);
    if (role === 'assistant' && !failed) div.appendChild(Chat.copyButton(content));
    return div;
  },

  /** A Copy button for an assistant reply: copies the reply's Markdown. */
  copyButton(content) {
    const copyBtn = Dom.node('button', null, 'chat-copy');
    copyBtn.innerHTML = ShellIcons.icon('copy') + '<span>Copy</span>';
    copyBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      navigator.clipboard.writeText(content).then(() => Chat.copied(copyBtn));
    });
    return copyBtn;
  },

  /** Says "Copied" on the button for a moment. */
  copied(copyBtn) {
    const label = copyBtn.querySelector('span');
    label.textContent = 'Copied';
    copyBtn.classList.add('done');
    setTimeout(() => {
      label.textContent = 'Copy';
      copyBtn.classList.remove('done');
    }, RendererConstants.COPIED_MS);
  },

  /** Shows the thinking indicator. */
  showThinking() {
    const list = Dom.byId('chat-messages');
    Chat.dropEmpty();
    const div = Dom.node('div', null, 'chat-thinking');
    div.innerHTML = '<div class="chat-dots"><span></span><span></span><span></span></div> Thinking…';
    list.appendChild(div);
    list.scrollTop = list.scrollHeight;
  },

  /** Records and shows one message; answers its element. */
  say(message) {
    Chat.history.push(message);
    return Chat.addChatMessage(message.role, message.content, message.toolCalls);
  },

  /** Sends the input to the agent and shows its reply or the error. */
  async send() {
    const input = Dom.byId('chat-input');
    const text = input.value.trim();
    if (!text || Chat.sending) return;
    input.value = '';
    Chat.grow();
    await Chat.exchange(text);
    input.focus();
  },

  /** One turn: show the question, then wait for the answer. */
  async exchange(text) {
    ChatPlaybook.withdraw();
    Chat.say({ role: 'user', content: ChatFiles.attachTo(text) });
    await Chat.turn();
  },

  /** Asks with the conversation as it stands, with the input locked until the answer comes. */
  async turn() {
    Chat.lock(true);
    Chat.showThinking();
    ChatProgress.start();
    await Chat.ask();
    ChatProgress.stop();
    Chat.lock(false);
  },

  /** Asks the agent with the whole conversation and shows the answer, unless the chat was cleared meanwhile. */
  async ask() {
    const epoch = Chat.epoch;
    const data = await Chat.request();
    if (epoch === Chat.epoch) Chat.answer(data);
  },

  /** The agent's answer to the conversation so far, with its files; a failure as `{ error }`. */
  async request() {
    try {
      return await oyaBrowser.sendChat(
        Chat.history.map((m) => ({ role: m.role, content: m.content })),
        ChatFiles.data(),
      );
    } catch (e) {
      return { error: e.message };
    }
  },

  /** Shows an answer: the reply, a stop, the model card when the project has no key, or the error. */
  answer(data) {
    if (CHAT_TEXT.keyCodes.includes(data.code)) ChatModel.needed(data.code === 'llm_rejected' && data.error);
    else if (data.error === CHAT_TEXT.stoppedError) Chat.say({ role: 'assistant', content: CHAT_TEXT.stopped });
    else if (data.error) Chat.sayError(data.error);
    else Chat.reply(data.text || CHAT_TEXT.noResponse, data.toolCalls || [], data.replayable);
  },

  /** Stops the agent's run: the main process hangs up, and the server ends the run at its next step. */
  stop() {
    oyaBrowser.stopChat();
  },

  /** Shows the agent's answer, offering to save the run as a playbook when the server says it can be. */
  reply(text, toolCalls, replayable) {
    const node = Chat.say({ role: 'assistant', content: text, toolCalls });
    const prompt = Chat.history.findLast((m) => m.role === 'user')?.content || '';
    ChatPlaybook.offer(node, prompt, toolCalls, replayable);
  },

  /** Shows an error as the assistant's reply. */
  sayError(message) {
    const errMsg = { role: 'assistant', content: 'Error: ' + message };
    Chat.history.push(errMsg);
    Chat.addChatMessage('assistant', errMsg.content);
  },

  /** Marks a message as in flight (or not): Stop stands in for Send, and the profile cannot change. */
  lock(on) {
    Chat.sending = on;
    Dom.byId('chat-send').hidden = on;
    Dom.byId('chat-stop').hidden = !on;
    ChatPersona.sync();
  },

  /** Fits the Ask box to its text, up to a limit, after which it scrolls. */
  grow() {
    const input = Dom.byId('chat-input');
    input.style.height = 'auto';
    input.style.height = Math.min(input.scrollHeight, RendererConstants.CHAT_INPUT_MAX_PX) + 'px';
  },

  /** Forgets the conversation and its files, stopping a run still going, and shows the empty state again. */
  clear() {
    if (Chat.sending) Chat.stop();
    Chat.epoch++;
    Chat.history = [];
    ChatFiles.clear();
    ChatPlaybook.withdraw();
    Dom.byId('chat-messages').innerHTML = Chat.emptyHtml;
  },

  /** Enter sends; Shift+Enter and IME composition insert text as usual. */
  keydown(e) {
    if (e.key !== 'Enter' || e.shiftKey || e.isComposing) return;
    e.preventDefault();
    Chat.send();
  },
};

Chat.emptyHtml = Dom.byId('chat-messages').innerHTML;
Dom.byId('chat-send').addEventListener('click', Chat.send);
Dom.byId('chat-stop').addEventListener('click', Chat.stop);
Dom.byId('chat-clear').addEventListener('click', Chat.clear);
Dom.byId('chat-input').addEventListener('keydown', Chat.keydown);
Dom.byId('chat-input').addEventListener('input', Chat.grow);
