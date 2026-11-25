/**
 * Pure display rules for the Playbooks tab: step descriptions, names and run
 * status text.
 */
import { MS_PER_SECOND, NAME_PATTERN, REPLY_PLACEHOLDER, SECONDS_PER_MINUTE, STATUS_CLASS } from './constants';
import type { AttentionReason, PlaybookRunRecord, RecordedStep, RunStatus } from './types';

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

/** How long a run took: "42s", or "3m 5s". */
export function runDuration(run: PlaybookRunRecord): string {
  const seconds = Math.max(0, Math.round((Date.parse(run.endedAt) - Date.parse(run.startedAt)) / MS_PER_SECOND));
  const minutes = Math.floor(seconds / SECONDS_PER_MINUTE);
  return minutes ? `${minutes}m ${seconds % SECONDS_PER_MINUTE}s` : `${seconds}s`;
}

/** A run's line: when it started, how long it took, and where. "9:00 AM · 42s · on b-1". */
export const runLine = (run: PlaybookRunRecord) =>
  `${new Date(run.startedAt).toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' })} · ${runDuration(run)} · on ${run.browserId}`;
