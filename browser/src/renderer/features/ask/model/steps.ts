/**
 * Pure helpers for a run's live line: what a browser command is doing in
 * words, how far along the run is, and the folded card's summary.
 */
import { RendererConstants as C } from '../../../core/constants.ts';
import { COMMAND_ENTRY, STEP_DETAILS, STEP_NAMES } from './constants.ts';
import type { DevLogEntry, RunUsage } from './types.ts';

/** A command entry's JSON: the command's parameters among its fields. */
interface CommandJson {
  /** What the action was given. */
  params?: Record<string, unknown>;
}

/** Whole seconds since `started`, never negative. */
export const elapsedSeconds = (started: number, now: number): number =>
  Math.max(0, Math.round((now - started) / C.MS_PER_SECOND));

/** A detail cut to the room the line has for it. */
export function shortDetail(text: string): string {
  const limit = C.CHAT_STEP_DETAIL_CHARS;
  return text.length > limit ? text.slice(0, limit) + '…' : text;
}

/** A command entry's parameters; none when its JSON was cut short. */
export function commandParams(entry: DevLogEntry): Record<string, unknown> {
  try {
    return (JSON.parse(entry.data ?? '') as CommandJson).params ?? {};
  } catch {
    return {};
  }
}

/** The action an activity entry is a command for, or undefined for anything else. */
export const commandAction = (entry: DevLogEntry): string | undefined =>
  entry.dir === 'in' ? COMMAND_ENTRY.exec(entry.type ?? '')?.[1] : undefined;

/** What an action is doing, with the thing it acts on where that helps. */
export function stepLabel(action: string, params: Record<string, unknown>): string {
  const name = Object.hasOwn(STEP_NAMES, action) ? STEP_NAMES[action] : action;
  const detail = Object.hasOwn(STEP_DETAILS, action) ? params[STEP_DETAILS[action]] : undefined;
  return detail ? `${name}, ${shortDetail(String(detail))}` : name;
}

/** How far along the answer is: steps so far and how long it has taken. */
export const progressMeta = (steps: number, seconds: number): string =>
  (steps ? `step ${steps} · ` : '') + `${seconds}s`;

/** A token count off the network: anything but a positive finite number is none. */
const tokenCount = (n: unknown): number => (typeof n === 'number' && Number.isFinite(n) && n > 0 ? n : 0);

/** A run's tokens in all, input and output together: the one figure a per-run budget is set on. */
const totalTokens = (usage?: RunUsage): number => tokenCount(usage?.input) + tokenCount(usage?.output);

/** Token totals written short: "840", "4.2K", "212K", "1.4M". */
const COMPACT = new Intl.NumberFormat(undefined, { notation: 'compact', maximumFractionDigits: 1 });

/** The summary's token figure, or '' when the run reported none: a made-up zero would mislead a budget. */
function tokensPart(usage?: RunUsage): string {
  const total = totalTokens(usage);
  return total ? ` · ${COMPACT.format(total)} token${total === 1 ? '' : 's'}` : '';
}

/** A finished run's summary: "1 step · 4s", "3 steps · 12s · 212K tokens". */
export const runSummary = (count: number, seconds: number, usage?: RunUsage): string =>
  `${count} step${count === 1 ? '' : 's'} · ${seconds}s${tokensPart(usage)}`;

/** The exact input and output split, for the summary's tooltip, since they are priced apart; '' with no tokens. */
export const usageTitle = (usage?: RunUsage): string =>
  totalTokens(usage)
    ? `${tokenCount(usage?.input).toLocaleString()} input · ${tokenCount(usage?.output).toLocaleString()} output tokens`
    : '';
