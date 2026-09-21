/**
 * Pure display rules for the Playbooks tab: step descriptions, names and run
 * status text.
 */
import { NAME_PATTERN, REPLY_PLACEHOLDER, STATUS_CLASS } from './constants';
import type { AttentionReason, RecordedStep, RunStatus } from './types';

/** How each kind of step reads, given the element's best label. */
const STEP_TEXT: Record<string, (s: RecordedStep, label: string) => string> = {
  navigate: (s) => `navigate ${s.url}`,
  type: (s, label) => `type ${label} ← ${s.text}`,
  select_option: (s, label) => `select ${label} ← ${s.option}`,
  press_key: (s) => `key ${s.key}`,
};

/** The element's most human name, or nothing. */
const elementLabel = ({ el }: RecordedStep) => (el ? el.text || el.name || el.domId || el.testId || el.tag || '' : '');

/** One recorded step as a line of the step list. */
export function describeStep(s: RecordedStep): string {
  const label = elementLabel(s);
  return Object.hasOwn(STEP_TEXT, s.action) ? STEP_TEXT[s.action](s, label) : `${s.action} ${label}`;
}

/** Whether a (trimmed) name is one the server accepts. */
export const isPlaybookName = (name: string) => NAME_PATTERN.test(name);

/** A typed address, with https:// added when it has no scheme. */
export const withScheme = (target: string) => (/^https?:\/\//i.test(target) ? target : `https://${target}`);

/** Whether a run is over. */
export const isEnded = (status: RunStatus | undefined) => status === 'succeeded' || status === 'failed';

/** The status text's colour class. */
export const statusClass = (status: RunStatus) => STATUS_CLASS[status] ?? 'text-accent';

/** The status as a person reads it. */
export const statusLabel = (status: RunStatus) => (status === 'needs_attention' ? 'needs a person' : status);

/** The reply box's hint for a paused run. */
export const replyPlaceholder = (reason: AttentionReason) => REPLY_PLACEHOLDER[reason] ?? 'done';

/** How old the live view's frame is, or null before the first one. Read at render, like the view's clock. */
export const frameAge = (frameAt: number | null) => (frameAt ? Date.now() - frameAt : null);
