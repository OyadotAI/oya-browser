/** Desktop stories follow one chore from asking for help to reusing successful steps. */
import type { AnswerSection } from './content';
/** The default start respects either the reader's existing plan or coding agent. */
export const DESKTOP_START: AnswerSection = {
  heading: 'Start with the task, not a script.',
  body: 'Type what you need in the Ask panel, using your ChatGPT or Claude plan or a model API key. If you already work with a coding agent, connect it to Oya instead.',
  items: [
    'Use your existing AI plan or a model API key in the Ask panel.',
    'Connect the coding agent you already use to Oya.',
  ],
  layout: 'choices',
  composition: 'start',
};
/** The rest of the journey explains separation, judgment, reuse and team scope in that order. */
export const DESKTOP_CHAPTERS: AnswerSection[] = [
  {
    heading: 'Let the agent work in its own tab.',
    body: 'Your tabs stay yours. The agent opens pages, clicks and fills forms in its own tab, using the browser’s existing logins.',
    composition: 'tabs',
    layout: 'controls',
  },
  {
    heading: 'Take the tab back when you need to.',
    body: 'Handle the part that needs you, then decide when the agent can continue. You can follow the work without handing over your whole browser.',
    composition: 'takeover',
  },
  {
    heading: 'Keep a successful task for next time.',
    body: 'Save its steps as a playbook: readable Playwright code with inputs you can change. Replay those steps without a model doing the task again.',
    composition: 'saved',
    layout: 'flow',
  },
  {
    heading: 'Move to a team deployment when the work grows.',
    body: 'Oya Cloud and the enterprise edition support teams running many browsers with an audit trail. HIPAA compliance and a signed Business Associate Agreement (BAA) apply to those covered editions. For work involving protected health information (PHI), use those covered editions.',
    composition: 'deployment',
    layout: 'choices',
  },
];
