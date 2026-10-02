/**
 * A chat run's live events: what the agent is doing, pushed to the browser that
 * asked over the socket it already holds, so its panel can show the plan and a
 * timeline of steps while the run goes on. Each event says where, never what:
 * typed text, chosen values and raw arguments stay out, and anything else is
 * redacted against the task's secrets. Sending is fire and forget; a browser that
 * cannot hear (gone, on another replica, an old app) never slows or breaks the run.
 */
import { randomUUID } from 'node:crypto';
import { registry } from '../browsers/registry.ts';
import { elementOf } from './recorder.ts';
import { redact } from './placeholders.ts';
import { EVENT_NAME_CHARS, EVENT_TEXT_CHARS, EVENT_PLAN_STEPS } from './constants.ts';

/** Shortens text past `max` with an ellipsis. */
const clip = (text: string, max: number) => (text.length > max ? `${text.slice(0, max - 1).trimEnd()}…` : text);

/** An element's name as the analysis read it: its label, text, placeholder or field name, on one line. */
function nameOf(element: any): string {
  const name = [element?.label, element?.text, element?.placeholder, element?.name].find((v) => String(v || '').trim());
  return name ? clip(String(name).replace(/\s+/g, ' ').trim(), EVENT_NAME_CHARS) : '';
}

/** A site's host name without www., or nothing when the address is not a URL. */
function hostOf(url: string): string {
  try {
    return new URL(url).hostname.replace(/^www\./, '');
  } catch {
    return '';
  }
}

/** A line about an element: its name in quotes when it has one, the plain verb when not. */
const onElement = (verb: string, plain: string) => (_args, name: string) => (name ? `${verb} “${name}”` : plain);

/** Each tool's line; a tool not listed here is not narrated. */
const LINES: Record<string, (args: any, name: string) => string> = {
  navigate: (args) => (hostOf(args.url) ? `Opening ${hostOf(args.url)}` : 'Opening a page'),
  click: onElement('Clicking', 'Clicking'),
  double_click: () => 'Double-clicking',
  click_coordinates: () => 'Clicking',
  type: onElement('Typing into', 'Typing'),
  keyboard_type: () => 'Typing',
  select_option: onElement('Choosing in', 'Choosing an option'),
  hover: onElement('Pointing at', 'Pointing'),
  upload_file: () => 'Attaching a file',
  press_key: (args) => `Pressing ${args.key || 'Enter'}`,
  scroll: () => 'Scrolling',
  drag: () => 'Dragging',
  go_back: () => 'Going back',
  go_forward: () => 'Going forward',
  reload: () => 'Reloading the page',
  analyze_page: () => 'Reading the page',
  read_elements: () => 'Reading the page',
  find: () => 'Looking for what it needs',
  screenshot: () => 'Looking at the page',
  run_script: () => 'Reading the page closely',
  wait_for: () => 'Waiting for the page',
  wait: () => 'Waiting for the page',
  open_tab: () => 'Opening a tab',
  switch_tab: () => 'Switching tabs',
  close_tab: () => 'Closing a tab',
  handle_dialog: () => 'Answering a dialog',
  request_human: () => 'Asking you',
  solve_captcha: () => 'Solving a CAPTCHA',
  sign_in: () => 'Signing in',
  complete_mfa: () => 'Entering the sign-in code',
};

/** The line for one tool call, naming its element when the analysis knew it; '' for a tool that is not narrated. */
export function stepLine(tool: string, args: any = {}, element?: any): string {
  if (!Object.hasOwn(LINES, tool)) return '';
  return LINES[tool](args || {}, nameOf(element));
}

/** A tool call as the loop reports it, before it runs. */
type ToolCall = {
  /** The tool's name. */
  name: string;
  /** Its arguments, as the model gave them. */
  args?: any;
};

/** How a run ended, as far as its events say. */
type Report = {
  /** Whether the report says the task could not be done. */
  failed?: boolean;
};

/** One chat run's events, sent to the browser that asked under one run id. */
export class LiveRun {
  /** The browser told. */
  browserId: string;
  /** The task's secrets, redacted out of everything sent. */
  secrets: Record<string, any>;
  /** Names this run's events, so the panel never mixes two runs. */
  runId = randomUUID();

  constructor(browserId: string, secrets: Record<string, any> = {}) {
    this.browserId = browserId;
    this.secrets = secrets || {};
  }

  /** The run began on `task`. */
  start(task: string) {
    this.send({ kind: 'start', task: this.safe(task) });
  }

  /** One tool call, about to run: the plan when it is the plan, a narrated step otherwise. */
  toolCall({ name, args }: ToolCall) {
    if (name === 'update_plan') return this.send({ kind: 'plan', steps: this.planOf(args?.steps) });
    const element = args?.element_id == null ? undefined : elementOf(this.browserId, args.element_id);
    const line = stepLine(name, args, element);
    if (line) this.send({ kind: 'step', tool: name, line: this.safe(line) });
  }

  /** The run ended with a report; `failed` when it says the task could not be done. */
  done(result: Report) {
    this.send({ kind: 'done', ok: !result?.failed });
  }

  /** The run threw before it could report. */
  failed(err: any) {
    this.send({ kind: 'failed', error: this.safe(err?.message || 'The run stopped') });
  }

  /** The plan's steps, each a redacted line and whether it is done. */
  planOf(steps: any) {
    const list = Array.isArray(steps) ? steps.slice(0, EVENT_PLAN_STEPS) : [];
    return list.map((s) => ({ step: this.safe(s?.step), done: !!s?.done }));
  }

  /** Text fit to send: secrets redacted, cut to a short length. */
  safe(text: any): string {
    return clip(redact(String(text ?? ''), this.secrets), EVENT_TEXT_CHARS);
  }

  /** Sends one event; never throws. */
  send(event: Record<string, any>) {
    try {
      registry.send(this.browserId, { type: 'agent-event', runId: this.runId, event });
    } catch {}
  }
}
