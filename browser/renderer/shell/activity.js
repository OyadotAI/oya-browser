/**
 * Whether an agent is working right now. Holding control is not the same as
 * working: a connected browser rests under agent control for hours. So the chrome's
 * signs of life (the window's orb turning, the thread of light along the toolbar,
 * the control capsule's ring) follow the commands actually arriving, and settle a
 * moment after the last one.
 */
/* global oyaBrowser, Dom, OyaOrb, RendererConstants */
/* exported AgentActivity */

/** A command from the server, as the activity log names it. */
const AGENT_COMMAND = /^cmd:/;

/** The agent's activity, as the chrome shows it. */
const AgentActivity = {
  /** The timer that settles the chrome after the last command. */
  timer: 0,

  /** One activity entry: a command arriving means the agent is at work. */
  note(entry) {
    if (entry?.dir !== 'in' || !AGENT_COMMAND.test(String(entry.type || ''))) return;
    AgentActivity.set(true);
    clearTimeout(AgentActivity.timer);
    AgentActivity.timer = setTimeout(() => AgentActivity.set(false), RendererConstants.AGENT_ACTIVE_MS);
  },

  /** Marks the root and turns (or rests) the window's orb. */
  set(active) {
    document.documentElement.dataset.agentActive = String(active);
    OyaOrb.state(Dom.byId('brand-orb'), active ? 'thinking' : 'idle');
  },
};

oyaBrowser.onDevLog(AgentActivity.note);
