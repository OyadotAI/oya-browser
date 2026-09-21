/**
 * What an activity entry may say about a command. Never the text that was
 * typed, this log is shown to whoever can see the dashboard.
 */
import { SUMMARY_CHARS } from '../constants.ts';

/** Reduces one command's params to a short, safe line. */
type Summary = (params: any) => string;

/** The URL being opened, trimmed. */
const url: Summary = (p) => String(p.url || '').slice(0, SUMMARY_CHARS);
/** How much was typed, never what. */
const typed: Summary = (p) => `${String(p.text || '').length} chars`;
/** The element aimed at. */
const target: Summary = (p) => String(p.selector || p.element_id || '');
/** A point on the page, or the element when no point was given. */
const point: Summary = (p) => (p.x !== undefined ? `${Math.round(p.x)},${Math.round(p.y)}` : target(p));
/** Where a drag started and ended. */
const drag: Summary = (p) =>
  `${Math.round(p.from_x)},${Math.round(p.from_y)} → ${Math.round(p.to_x)},${Math.round(p.to_y)}`;
/** The tab switched to or closed. */
const tab: Summary = (p) => String(p.tab_id || '');

/** The summary for each action; any other action says nothing. */
const SUMMARIES: Record<string, Summary> = {
  navigate: url,
  open_tab: url,
  type: typed,
  keyboard_type: typed,
  press_key: (p) => String(p.key || ''),
  click_coordinates: point,
  mouse_move: point,
  double_click: point,
  hover: point,
  drag,
  click: target,
  select: target,
  wait: target,
  scroll: (p) => `${p.direction || 'down'} ${p.amount || ''}`.trim(),
  switch_tab: tab,
  close_tab: tab,
};

/** The activity line for `action`, or '' when there is nothing safe to say. */
export function summarise(action, params: any = {}) {
  if (!params || typeof params !== 'object') return '';
  return Object.hasOwn(SUMMARIES, action) ? SUMMARIES[action](params) : '';
}
