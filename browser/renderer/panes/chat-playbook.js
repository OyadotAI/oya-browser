/**
 * "Save as playbook" under an agent reply. The server keeps the steps of each
 * browser's latest agent run, so only the newest reply offers to save, and only
 * when that run acted on the page.
 */
/* global oyaBrowser, Dom, ShellIcons, RendererConstants */
/* exported ChatPlaybook */

/**
 * Tools the server records as replayable steps (server/src/modules/agent/recorder.ts,
 * RECORDED). A run that used none of them has nothing to replay.
 */
const REPLAYABLE_TOOLS = new Set([
  'navigate',
  'click',
  'type',
  'select_option',
  'upload_file',
  'press_key',
  'scroll',
  'wait',
  'handle_dialog',
]);

/** A playbook name the server accepts. */
const PLAYBOOK_NAME = /^[\w-]{1,64}$/;

/** The save-as-playbook offer. */
const ChatPlaybook = {
  /** Adds the offer under `message` when the run's tool calls can be replayed. */
  offer(message, prompt, toolCalls) {
    ChatPlaybook.withdraw();
    if (!toolCalls.some((call) => REPLAYABLE_TOOLS.has(call.name))) return;
    const box = Dom.node('div', null, 'chat-save');
    box.dataset.name = ChatPlaybook.suggestName(prompt);
    message.appendChild(box);
    ChatPlaybook.showButton(box);
  },

  /** Removes the offer: a new run replaces the one the server would save. */
  withdraw() {
    document.querySelectorAll('.chat-save:not(.saved)').forEach((box) => box.remove());
  },

  /** A name from the prompt: lowercase words joined by hyphens. */
  suggestName(prompt) {
    const slug = prompt.toLowerCase().replace(/[^a-z0-9]+/g, '-');
    return slug.slice(0, RendererConstants.PLAYBOOK_NAME_SUGGESTION).replace(/^-+|-+$/g, '') || 'agent-run';
  },

  /** The collapsed offer: one button. */
  showButton(box) {
    const button = Dom.node('button', null, 'chat-save-button');
    button.innerHTML = ShellIcons.icon('playbook') + '<span>Save as playbook</span>';
    button.addEventListener('click', () => ChatPlaybook.showForm(box));
    box.replaceChildren(button);
  },

  /** The name field with Save and Cancel. */
  showForm(box) {
    const { input, save, cancel } = ChatPlaybook.controls(box);
    const error = Dom.node('p', '', 'chat-save-error');
    box.replaceChildren(ChatPlaybook.explainer(), ChatPlaybook.row(input, save, cancel), error);
    input.select();
  },

  /** The form's name field, Save and Cancel, wired: Enter saves, Escape cancels. */
  controls(box) {
    const input = ChatPlaybook.nameField(box.dataset.name);
    const save = Dom.node('button', 'Save', 'chat-save-confirm');
    const cancel = Dom.node('button', 'Cancel', 'chat-save-cancel');
    save.addEventListener('click', () => ChatPlaybook.save(box, input.value.trim()));
    cancel.addEventListener('click', () => ChatPlaybook.showButton(box));
    input.addEventListener('keydown', (e) => ChatPlaybook.formKey(e, save, cancel));
    return { input, save, cancel };
  },

  /** What saving does. */
  explainer() {
    return Dom.node('p', 'Save this run as a playbook to replay it without the model.', 'chat-save-copy');
  },

  /** The playbook name input. */
  nameField(name) {
    const input = Dom.node('input', null, 'chat-save-name');
    Object.assign(input, { value: name, maxLength: 64, spellcheck: false });
    input.setAttribute('aria-label', 'Playbook name');
    return input;
  },

  /** The form's controls in one row. */
  row(...controls) {
    const row = Dom.node('div', null, 'chat-save-row');
    row.append(...controls);
    return row;
  },

  /** Enter presses Save and Escape presses Cancel. */
  formKey(event, save, cancel) {
    if (event.key === 'Enter' && !event.isComposing) save.click();
    else if (event.key === 'Escape') cancel.click();
    else return;
    event.preventDefault();
  },

  /** Saves the run under `name`, or explains why it could not. */
  async save(box, name) {
    if (!PLAYBOOK_NAME.test(name)) return ChatPlaybook.fail(box, 'Use 1–64 letters, numbers, hyphens or underscores.');
    box.querySelectorAll('button, input').forEach((control) => (control.disabled = true));
    const result = await oyaBrowser.saveChatPlaybook(name).catch((e) => ({ error: e.message }));
    if (result.error) return ChatPlaybook.fail(box, result.error);
    ChatPlaybook.saved(box, name);
  },

  /** Shows why saving failed and lets the person try again. */
  fail(box, message) {
    box.querySelectorAll('button, input').forEach((control) => (control.disabled = false));
    box.querySelector('.chat-save-error').textContent = message;
    box.querySelector('.chat-save-name').focus();
  },

  /** The saved confirmation, which stays after later runs. */
  saved(box, name) {
    box.classList.add('saved');
    const note = Dom.node('p', null, 'chat-save-done');
    note.innerHTML = ShellIcons.icon('check') + '<span></span>';
    note.querySelector('span').textContent = `Saved as playbook “${name}”`;
    box.replaceChildren(note);
  },
};
