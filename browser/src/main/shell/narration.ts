/**
 * What the control shield's companion says while an agent works: one short line
 * per action, read off the action and the element's name from the last analysis.
 * It never repeats what was typed or chosen, because a value can be a password or
 * someone's personal details; it says where, not what. Elements are matched by their
 * analysis id: the page script's own attribute name is randomised, so the selector the
 * server sends (`[data-ac-id="16"]`) never equals the one the analysis reported.
 */
import { NARRATION_NAME_MAX } from './constants.ts';

/** One element as the page analysis reported it. */
export interface AnalyzedElement {
  /** Its analysis id, the number the agent aims actions with. */
  id: number;
  /** Its kind (link, button, input, select ...), which colours its outline. */
  type?: string;
  /** A selector that finds it again. */
  selector?: string;
  /** Whether the analysis saw it on screen. */
  visible?: boolean;
  /** Its accessible label. */
  label?: string;
  /** Its visible text. */
  text?: string;
  /** A field's placeholder. */
  placeholder?: string;
  /** A field's form name. */
  name?: string;
}

/** The parameters of an agent action the companion reads. */
export interface ActionParams {
  /** Where a navigate goes. */
  url?: string;
  /** The key a press_key presses. */
  key?: string;
  /** The element an action is aimed at, as a selector. */
  selector?: string;
  /** The element an action is aimed at, as an analysis id. */
  element_id?: number | string | null;
}

/** Analysis id to the element's name. */
export type ElementNames = Map<number, string>;

/** Builds one action's line from its parameters and the named element's name ('' when unnamed). */
type Line = (params: ActionParams | undefined, name: string) => string;

/** An element's name as the analysis read it: its label, its text, its placeholder or its field name. */
export function nameOf(element: Partial<AnalyzedElement> | null | undefined): string {
  const name = [element?.label, element?.text, element?.placeholder, element?.name].find((v) => String(v || '').trim());
  return name ? clip(String(name).replace(/\s+/g, ' ').trim()) : '';
}

/** Shortens a long name with an ellipsis, so the line stays one glance long. */
function clip(text: string): string {
  return text.length > NARRATION_NAME_MAX ? `${text.slice(0, NARRATION_NAME_MAX - 1).trimEnd()}…` : text;
}

/** The site's host name, without www., or the address itself when it is not a URL. */
function hostOf(url: string | undefined): string {
  try {
    return new URL(String(url)).hostname.replace(/^www\./, '');
  } catch {
    return clip(String(url || ''));
  }
}

/** A line about an element: with its name in quotes when it has one, the plain verb when not. */
const onElement =
  (verb: string, plain: string): Line =>
  (_params, name) =>
    name ? `${verb} “${name}”` : plain;

/** Each action's line, by action name; an action not listed here says nothing. */
const LINES: Record<string, Line> = {
  navigate: (params) => `Opening ${hostOf(params?.url)}`,
  click: onElement('Clicking', 'Clicking'),
  double_click: () => 'Double-clicking',
  click_coordinates: () => 'Clicking',
  type: onElement('Typing into', 'Typing'),
  keyboard_type: () => 'Typing',
  select: onElement('Choosing in', 'Choosing an option'),
  hover: onElement('Pointing at', 'Pointing'),
  press_key: (params) => `Pressing ${params?.key || 'Enter'}`,
  scroll: () => 'Scrolling',
  drag: () => 'Dragging',
  back: () => 'Going back',
  forward: () => 'Going forward',
  reload: () => 'Reloading the page',
  screenshot: () => 'Looking at the page',
  run_script: () => 'Reading the page closely',
};

/** Actions that change the page, so whatever the shield was showing about it is out of date. */
const PAGE_CHANGING = new Set([
  'navigate',
  'click',
  'double_click',
  'click_coordinates',
  'type',
  'keyboard_type',
  'select',
  'press_key',
  'drag',
  'back',
  'forward',
  'reload',
]);

/** Whether `action` changes the page. */
export function changesPage(action: string): boolean {
  return PAGE_CHANGING.has(action);
}

/** The analysis id an action is aimed at, from its element_id or its `[data-…="N"]` selector, or null. */
export function idOf(params: ActionParams | undefined): number | null {
  const id = params?.element_id ?? /^\[data-[\w-]+="(\d+)"\]$/.exec(String(params?.selector || ''))?.[1];
  return id === undefined || id === null || id === '' ? null : Number(id);
}

/** The companion's line for one action, or '' when it has nothing to say about it. `names` maps an analysis id to its element's name. */
export function lineFor(action: string, params: ActionParams | undefined, names: ElementNames): string {
  if (!Object.hasOwn(LINES, action)) return '';
  const id = idOf(params);
  return LINES[action](params, (id !== null && names.get(id)) || '');
}

/** Analysis id to name, for every element an analysis found. */
export function namesFrom(elements: readonly (AnalyzedElement | null | undefined)[] | null | undefined): ElementNames {
  const known = (elements || []).filter((e): e is AnalyzedElement => Number.isFinite(e?.id));
  return new Map(known.map((e) => [e.id, nameOf(e)]));
}
