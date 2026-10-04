/**
 * Pure helpers for the model picker: OpenRouter's "Vendor: Model" names, the
 * search (every word must match a model's name or id), the parts of a name a
 * search marks, and what the list and its button say.
 */
import { ASK_TEXT } from './constants.ts';
import type { ModelOption } from './types.ts';

/** A model's vendor ('' when it has none) and its name. */
export interface ModelLabel {
  /** The vendor, the heading it is grouped under. */
  group: string;
  /** The model's own name. */
  name: string;
}

/** One run of a name: matched by the search or not. */
export interface MarkPart {
  /** The text. */
  text: string;
  /** Whether a searched word matched it. */
  mark: boolean;
}

/** The picker button's two lines. */
export interface TriggerText {
  /** The chosen model's name. */
  name: string;
  /** What sits under it: its id, its vendor, or that it is a custom id. */
  detail: string;
}

/** "Vendor: Model" (OpenRouter's naming) as its vendor and model name; other names have no vendor. */
export function splitModelLabel(label: string): ModelLabel {
  const [group, ...rest] = label.split(': ');
  return rest.length && group ? { group, name: rest.join(': ') } : { group: '', name: label };
}

/** The words of a search, lowercased. */
export const searchWords = (query: string): string[] => query.toLowerCase().split(/\s+/).filter(Boolean);

/** The models matching `query`, then the query itself as a custom id when no model has exactly that id. */
export function matchingModels(models: readonly ModelOption[], query: string): ModelOption[] {
  const words = searchWords(query);
  const hits = models.filter((m) => words.every((w) => `${m.label} ${m.id}`.toLowerCase().includes(w)));
  const custom = query && !models.some((m) => m.id === query) ? [{ id: query, label: query, custom: true }] : [];
  return [...hits, ...custom];
}

/** `text` split into the runs a search for `words` marks and the runs it does not. */
export function markParts(text: string, words: readonly string[]): MarkPart[] {
  if (!words.length) return [{ text, mark: false }];
  const escaped = words.map((w) => w.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'));
  // Splitting on a captured pattern alternates: text, match, text, match, …
  const parts = text.split(new RegExp(`(${escaped.join('|')})`, 'i'));
  return parts.map((part, i) => ({ text: part, mark: (i & 1) === 1 })).filter((p) => p.text);
}

/** The chosen model, or an entry for an id the list does not have. */
export function chosenModel(models: readonly ModelOption[], value: string): ModelOption {
  return models.find((m) => m.id === value) ?? { id: value, label: value || ASK_TEXT.chooseModel, custom: !!value };
}

/** The picker button's two lines: the chosen model's name, and its id underneath when that says more. */
export function triggerText(models: readonly ModelOption[], value: string): TriggerText {
  const model = chosenModel(models, value);
  const { group, name } = splitModelLabel(model.label);
  return { name, detail: model.custom ? ASK_TEXT.customModel : model.id !== name ? model.id : group };
}

/** Whether the options come from more than one vendor, so the list is grouped. */
export const isGrouped = (shown: readonly ModelOption[]): boolean =>
  new Set(shown.filter((o) => !o.custom).map((o) => splitModelLabel(o.label).group)).size > 1;

/** The vendor heading above option `index`, when it is the first of its vendor in a grouped list. */
export function groupHeading(shown: readonly ModelOption[], index: number, grouped: boolean): string {
  const option = shown[index];
  const group = splitModelLabel(option.label).group;
  const previous = index ? splitModelLabel(shown[index - 1].label).group : null;
  return !grouped || option.custom || !group || group === previous ? '' : group;
}

/** "1 model", "3 models": the listed models, a typed id not counted. */
export function modelCount(shown: readonly ModelOption[]): string {
  const models = shown.filter((o) => !o.custom).length;
  return `${models} ${models === 1 ? 'model' : 'models'}`;
}
