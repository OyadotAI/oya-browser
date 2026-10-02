/**
 * The live run in the Ask pane. While the agent works, its card under the question
 * shows the plan it wrote, ticking off steps, and a timeline of what it did, one
 * narrated line per action, from the server's agent events. With a server that sends
 * none, the live line under it still follows the browser's commands (chat-progress.js).
 * When the answer comes, the card folds into a one-line summary that opens again.
 */
/* global oyaBrowser, Dom, RendererConstants, OyaOrb, ChatProgress */
/* exported ChatRun */

/** The live run card and the panel's orb. */
const ChatRun = {
  /** The card of the run in flight, or null. */
  card: null,
  /** Whether this run has had agent events (then they, not the socket commands, name the steps). */
  live: false,
  /** When the run began. */
  started: 0,
  /** The server's id for this run, from its start event; events of any other run are ignored. */
  runId: null,
  /** The timer that rests the orb after a run. */
  restTimer: 0,

  /** A question was asked: the card goes under it, with its live line, and the orb starts thinking. */
  begin() {
    const card = Dom.node('div', null, 'chat-run');
    card.innerHTML = '<ol class="run-plan" hidden></ol><ol class="run-steps"></ol><div class="chat-thinking"></div>';
    Object.assign(ChatRun, { card, live: false, started: Date.now(), runId: null });
    Dom.byId('chat-messages').appendChild(card);
    ChatRun.orb('thinking');
    return card;
  },

  /**
   * One event from the agent while it runs. The run's own start event names it;
   * until then, and for any other run (one just stopped, or another client's), events
   * are ignored, so two runs never mix in one card.
   */
  event({ runId, event } = {}) {
    if (!ChatRun.card || !event) return;
    if (event.kind === 'start' && !ChatRun.runId) ChatRun.runId = runId;
    if (!ChatRun.runId || runId !== ChatRun.runId || !Object.hasOwn(ChatRun.EVENTS, event.kind)) return;
    ChatRun.live = true;
    ChatRun.EVENTS[event.kind](event);
  },

  /** What each kind of event does to the card. */
  EVENTS: {
    plan: (event) => ChatRun.plan(event.steps || []),
    step: (event) => ChatRun.step(event.line || event.tool || ''),
  },

  /** Redraws the plan: each step, ticked when done. */
  plan(steps) {
    const list = ChatRun.card.querySelector('.run-plan');
    list.hidden = !steps.length;
    list.replaceChildren(...steps.map((s) => Dom.node('li', String(s.step || ''), s.done ? 'done' : '')));
  },

  /** Adds one narrated action to the timeline, with when it happened, and lets the orb pulse. */
  step(line) {
    const row = Dom.node('li', null, 'run-step');
    row.append(Dom.node('span', line, 'run-step-line'), Dom.node('span', ChatRun.elapsed(), 'run-step-time'));
    const steps = ChatRun.card.querySelector('.run-steps');
    steps.append(row);
    // The live line is what comes next, not a copy of the row just added; its count is the rows so far.
    Object.assign(ChatProgress, { label: '', steps: steps.children.length });
    ChatProgress.draw();
    OyaOrb.pulse(Dom.byId('panel-orb'));
    ChatRun.scroll();
  },

  /** The answer came: the live line goes, the card folds into a summary, and the orb shows how it went. */
  finish(ok) {
    const card = ChatRun.card;
    if (!card) return;
    card.querySelector('.chat-thinking')?.remove();
    card.classList.add(ok ? 'done' : 'failed');
    ChatRun.fold(card);
    ChatRun.card = null;
    ChatRun.settle(ok);
  },

  /** Folds a finished card behind its summary; a run with neither steps nor a plan leaves no card. */
  fold(card) {
    const count = card.querySelectorAll('.run-step').length;
    if (count) card.prepend(ChatRun.summary(card, count));
    else if (card.querySelector('.run-plan').hidden) card.remove();
  },

  /** The chat was cleared mid-run: the card is gone with it, and the orb rests. */
  reset() {
    Object.assign(ChatRun, { card: null, live: false, runId: null });
    clearTimeout(ChatRun.restTimer);
    ChatRun.orb('idle');
  },

  /** The folded card's summary line, which opens and closes the steps. */
  summary(card, count) {
    const button = Dom.node('button', `${count} step${count === 1 ? '' : 's'} · ${ChatRun.elapsed()}`, 'run-summary');
    button.type = 'button';
    button.setAttribute('aria-expanded', 'false');
    button.addEventListener('click', () => ChatRun.toggle(card, button));
    card.classList.add('folded');
    return button;
  },

  /** Opens or folds a finished run's steps. */
  toggle(card, button) {
    const folded = card.classList.toggle('folded');
    button.setAttribute('aria-expanded', String(!folded));
  },

  /** The orb shows done or failed for a moment, then rests. */
  settle(ok) {
    ChatRun.orb(ok ? 'done' : 'failed');
    clearTimeout(ChatRun.restTimer);
    ChatRun.restTimer = setTimeout(() => ChatRun.orb('idle'), RendererConstants.CHAT_ORB_REST_MS);
  },

  /** Sets the panel orb's state. */
  orb(state) {
    OyaOrb.state(Dom.byId('panel-orb'), state);
  },

  /** How long the run has taken, as people read it. */
  elapsed() {
    return `${Math.max(0, Math.round((Date.now() - ChatRun.started) / RendererConstants.MS_PER_SECOND))}s`;
  },

  /** Keeps the newest step in view. */
  scroll() {
    const list = Dom.byId('chat-messages');
    list.scrollTop = list.scrollHeight;
  },
};

oyaBrowser.onAgentEvent?.(ChatRun.event);
