/**
 * The element index appended to an analysis, written as TOON tables
 * (see toon.ts): every form field with its label, value, expected format and
 * state; the other visible elements; and a capped list of off-screen ones. The
 * agent fills forms from the field table, so it carries what the page shows
 * about each field: required, invalid and the page's error for it.
 */
import { MAX_OFFSCREEN_LISTED, MAX_INDEX_LINK } from './constants.ts';
import { toonTable } from './toon.ts';

/** Element kinds a person fills in rather than clicks. */
const FIELD_TYPES = new Set(['input', 'textarea', 'select', 'checkbox', 'radio', 'editable']);

/** The field table's columns. */
const FIELD_COLUMNS = ['id', 'type', 'label', 'value', 'hint', 'state'];

/** The visible-element table's columns. */
const VISIBLE_COLUMNS = ['id', 'type', 'label', 'link'];

/** What the field table is, said once above it. */
const FIELDS_NOTE = 'Form fields (hint is the format the field expects, or a select’s options):\n';

/** Said after the index when the page was cut short. */
const TRUNCATED = '\n⚠ Page content was truncated (very long page). Scroll down and re-analyze to see more.\n';

/** A checkbox or radio, whose value is its state rather than text. */
const isToggle = (e) => e.type === 'checkbox' || e.type === 'radio';

/** The field's kind: an input by its input type (text, date, password), anything else by its type. */
const fieldKind = (e) => (e.type === 'input' ? e.inputType || 'text' : e.type);

/** State words: required, checked, disabled, read-only, off-screen, and invalid with the page's error. */
function fieldState(e) {
  const parts = [e.required && 'required', isToggle(e) && (e.checked ? 'checked' : 'unchecked')];
  parts.push(e.disabled && 'disabled', e.readOnly && 'readonly', !e.visible && 'off-screen');
  if (e.invalid || e.error) parts.push(e.error ? `invalid: ${e.error}` : 'invalid');
  return parts.filter(Boolean).join(' ');
}

/** The format a field expects, or a select's options; blank when it only repeats the label. */
function fieldHint(e) {
  const hint = e.options || e.placeholder || '';
  return hint === e.text ? '' : hint;
}

/** One row of the field table. */
const fieldRow = (e) => ({
  id: e.id,
  type: fieldKind(e),
  label: e.text,
  value: isToggle(e) ? '' : e.value,
  hint: fieldHint(e),
  state: fieldState(e),
});

/** One row for a visible element that is not a field. */
const visibleRow = (e) => ({
  id: e.id,
  type: e.type,
  label: e.disabled ? `${e.text || ''} (disabled)` : e.text,
  link: (e.href || '').slice(0, MAX_INDEX_LINK),
});

/** The first off-screen elements, and a count of the rest. */
function offscreenSection(offscreen) {
  const rows = offscreen.slice(0, MAX_OFFSCREEN_LISTED).map((e) => ({ id: e.id, type: e.type, label: e.text }));
  let section = '\nOff-screen (scroll to reveal):\n' + toonTable('offscreen', ['id', 'type', 'label'], rows) + '\n';
  if (offscreen.length > MAX_OFFSCREEN_LISTED)
    section += `  ... and ${offscreen.length - MAX_OFFSCREEN_LISTED} more off-screen elements\n`;
  return section;
}

/** Every field, the visible ones first: the whole form in one table. */
function fieldsSection(elements) {
  const fields = elements.filter((e) => FIELD_TYPES.has(e.type));
  if (!fields.length) return '';
  fields.sort((a, b) => Number(!a.visible) - Number(!b.visible));
  return FIELDS_NOTE + toonTable('fields', FIELD_COLUMNS, fields.map(fieldRow)) + '\n';
}

/** The visible elements that are not fields. */
function visibleSection(others) {
  const visible = others.filter((e) => e.visible);
  if (!visible.length) return '';
  return '\nOther visible elements:\n' + toonTable('visible', VISIBLE_COLUMNS, visible.map(visibleRow)) + '\n';
}

/** The index for an analysis's elements, noting when the page content was truncated. */
export function elementIndex(elements, truncated) {
  const others = elements.filter((e) => !FIELD_TYPES.has(e.type));
  const offscreen = others.filter((e) => !e.visible);
  let index = `\n\n## Element Index (${elements.length} total, ${elements.filter((e) => e.visible).length} visible)\n\n`;
  index += fieldsSection(elements) + visibleSection(others);
  if (offscreen.length) index += offscreenSection(offscreen);
  return truncated ? index + TRUNCATED : index;
}
