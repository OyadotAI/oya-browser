/**
 * How the Actions pane shows what an action answered: a heading with the
 * action, whether it worked and when; then a screenshot, an analysis (with
 * the page's elements to pick from), data as JSON, or the error, cut to size
 * with a note saying so. Pure functions of the answer.
 */
import { RendererConstants as C } from '../../../core/constants.ts';
import { ACTION_CHECKS, ACTION_TITLES, ACTIONS_TEXT } from './constants.ts';

/** What the result box shows. */
export interface ActionResult {
  /** "Reload · done · 10:02:03". */
  title: string;
  /** The action failed (the box turns red). */
  failed: boolean;
  /** The text shown, cut to size; '' for a screenshot. */
  text: string;
  /** The text was cut, so the note says Copy takes all of it. */
  cut: boolean;
  /** A screenshot's data URL, or ''. */
  image: string;
}

/** One element of an analyzed page, to pick its number from. */
export interface ElementItem {
  /** Its number. */
  id: string;
  /** "#12 button Sign in". */
  label: string;
}

/** What an answer becomes: the box, the text Copy takes, and the elements an analysis found. */
export interface ShownResult {
  /** The result box. */
  result: ActionResult;
  /** Everything Copy takes. */
  copyText: string;
  /** The elements to pick from (an analysis only). */
  elements?: ElementItem[];
}

/** An answer's fields, as far as the pane reads them. */
interface Answer {
  /** It worked. */
  ok?: boolean;
  /** Why not. */
  error?: unknown;
  /** What came back. */
  data?: Record<string, unknown>;
}

/** The first problem with `params`, or ''. */
export function problemWith(params: Record<string, string>): string {
  for (const [key, value] of Object.entries(params)) {
    const message = Object.hasOwn(ACTION_CHECKS, key) ? ACTION_CHECKS[key](value) : '';
    if (message) return message;
  }
  return '';
}

/** A result's heading: the action, whether it worked, and when. */
export function heading(action: string, ok: boolean, at: Date): string {
  const title = Object.hasOwn(ACTION_TITLES, action)
    ? ACTION_TITLES[action]
    : action[0].toUpperCase() + action.slice(1);
  return `${title} · ${ok ? ACTIONS_TEXT.done : ACTIONS_TEXT.failed} · ${at.toLocaleTimeString()}`;
}

/** `text` in the box, cut to `limit` characters. */
function textResult(title: string, failed: boolean, text: string, limit = Infinity): ShownResult {
  const cut = text.length > limit;
  return { result: { title, failed, text: cut ? text.slice(0, limit) : text, cut, image: '' }, copyText: text };
}

/** The analyzed page's visible elements, as many as the pane lists. */
export function elementsOf(raw: unknown): ElementItem[] {
  const list = Array.isArray(raw) ? (raw as Record<string, unknown>[]) : [];
  const shown = list.filter((e) => e?.visible !== false).slice(0, C.ACTION_ELEMENTS_SHOWN);
  return shown.map((e) => ({ id: String(e.id), label: `#${e.id} ${e.type} ${e.text || ''}`.trim() }));
}

/** An analysis: its page text, and its elements to pick from. */
function analysisResult(title: string, data: Record<string, unknown>): ShownResult {
  const page = typeof data.page === 'string' ? data.page : '';
  return { ...textResult(title, false, page, C.ANALYZE_PREVIEW), elements: elementsOf(data.elements) };
}

/** A screenshot; Copy takes its data URL. */
const shotResult = (title: string, image: string): ShownResult => ({
  result: { title, failed: false, text: '', cut: false, image },
  copyText: image,
});

/** Shows an answer under its heading: a screenshot, an analysis, data, or the error. */
export function showResult(action: string, answer: Answer | null | undefined, at: Date): ShownResult {
  const title = heading(action, !!answer?.ok, at);
  if (!answer?.ok) return textResult(title, true, String(answer?.error || ACTIONS_TEXT.unknownError));
  const data = answer.data ?? {};
  if (action === 'screenshot' && typeof data.screenshot === 'string') return shotResult(title, data.screenshot);
  if (action === 'analyze') return analysisResult(title, data);
  return textResult(title, false, JSON.stringify(answer.data || answer, null, C.JSON_INDENT), C.RESULT_PREVIEW);
}
