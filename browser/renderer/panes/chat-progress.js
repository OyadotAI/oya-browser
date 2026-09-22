/**
 * The live step line in the Ask pane. Every tool call the agent makes reaches
 * this browser as a control-socket command, which the main process already
 * mirrors to the renderer for the Activity pane; while a question is in flight
 * the Ask pane shows the latest one instead of a bare "Thinking…".
 *
 * ponytail: these are socket commands, not the model's tool calls, one `click`
 * tool shows "Click" and then "Reading the page", because the server re-analyses
 * the page after an action, and a command another client sends to this browser
 * mid-question counts as a step. If that is ever not close enough, stream the
 * real tool calls as NDJSON through the chat response, which is already held
 * open for the whole run.
 */
/* global oyaBrowser, Dom, RendererConstants, Studio */
/* exported ChatProgress */

/** An activity entry for a command from the server: `cmd: <action>`. */
const COMMAND_ENTRY = /^cmd:\s*(\S+)$/;

/** What people are told an action is doing, for the ones Studio does not name. */
const STEP_NAMES = {
  analyze: 'Reading the page',
  screenshot: 'Taking a screenshot',
  list_tabs: 'Listing the tabs',
  open_tab: 'Opening a tab',
  switch_tab: 'Switching tab',
  close_tab: 'Closing a tab',
  handle_dialog: 'Answering a dialog',
  click_coordinates: 'Clicking',
  double_click: 'Double-clicking',
  keyboard_type: 'Typing',
  mouse_move: 'Moving the mouse',
  drag: 'Dragging',
  hover: 'Hovering',
  back: 'Going back',
  forward: 'Going forward',
  reload: 'Reloading the page',
  run_script: 'Reading the page',
};

/** The parameter that says what an action acted on, for actions where it reads well. */
const STEP_DETAILS = {
  navigate: 'url',
  open_tab: 'url',
  type: 'text',
  keyboard_type: 'text',
  wait: 'selector',
  press_key: 'key',
  scroll: 'direction',
};

/** The step line shown under a question while the agent works on it. */
const ChatProgress = {
  /** A question is in flight. */
  running: false,
  /** When it was asked. */
  started: 0,
  /** Commands seen since then. */
  steps: 0,
  /** What the latest command is doing, in the words people see. */
  label: '',
  /** The timer that keeps the elapsed count moving between commands. */
  ticker: undefined,

  /** Starts the line under the question just asked. */
  start() {
    Object.assign(ChatProgress, { running: true, started: Date.now(), steps: 0, label: '' });
    ChatProgress.ticker = setInterval(ChatProgress.draw, RendererConstants.CHAT_PROGRESS_TICK_MS);
    ChatProgress.draw();
  },

  /** Stops the line; the reply that replaces it removes it. */
  stop() {
    ChatProgress.running = false;
    clearInterval(ChatProgress.ticker);
    ChatProgress.ticker = undefined;
  },

  /** Counts one command the server sent while a question is in flight. */
  note(entry) {
    if (!ChatProgress.running || entry.dir !== 'in') return;
    const action = COMMAND_ENTRY.exec(entry.type)?.[1];
    if (!action) return;
    ChatProgress.steps++;
    ChatProgress.label = ChatProgress.labelFor(action, ChatProgress.params(entry));
    ChatProgress.draw();
  },

  /** A command entry's parameters; none when its JSON was truncated. */
  params(entry) {
    try {
      return JSON.parse(entry.data).params || {};
    } catch {
      return {};
    }
  },

  /** What an action is doing, with the thing it acts on where that helps. */
  labelFor(action, params) {
    const name = Object.hasOwn(STEP_NAMES, action) ? STEP_NAMES[action] : Studio.name(action);
    const detail = Object.hasOwn(STEP_DETAILS, action) ? params[STEP_DETAILS[action]] : undefined;
    return detail ? `${name}, ${ChatProgress.short(String(detail))}` : name;
  },

  /** A detail cut to the room the line has for it. */
  short(text) {
    const limit = RendererConstants.CHAT_STEP_DETAIL_CHARS;
    return text.length > limit ? text.slice(0, limit) + '…' : text;
  },

  /** How far along the answer is: steps so far and how long it has taken. */
  meta() {
    const seconds = Math.round((Date.now() - ChatProgress.started) / RendererConstants.MS_PER_SECOND);
    return (ChatProgress.steps ? `step ${ChatProgress.steps} · ` : '') + `${seconds}s`;
  },

  /** Redraws the line, if it is still on the page. */
  draw() {
    const line = Dom.byId('chat-messages').querySelector('.chat-thinking');
    if (!line) return;
    line.innerHTML =
      '<div class="chat-dots"><span></span><span></span><span></span></div>' +
      `<span class="chat-step-label">${Dom.esc(ChatProgress.label || 'Thinking…')}</span>` +
      `<span class="chat-step-meta">${Dom.esc(ChatProgress.meta())}</span>`;
  },
};

oyaBrowser.onDevLog(ChatProgress.note);
