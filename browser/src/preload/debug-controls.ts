/** Baseline control semantics and accessible names remain available with private diagnostic node IDs. */
import { BROWSER_DEBUG as C } from '../shared/browser-debug.ts';
import '../../scripts/analyzer.js';
/** This hook is statically initialized in the private content world with all observer/recorder wiring skipped. */
interface Semantics {
  /** Canonical classification. */ type(node: Element): string | null;
  /** Canonical accessible/contextual name. */ name(node: Element, type: string): string;
  /** Visible proxy retains the underlying field's state. */ control(node: Element): Element;
  /** Checked state of native/custom controls. */ checked(node: Element): boolean | undefined;
  /** Radio position in its group. */ choice(node: Element): string;
  /** Expanded/selected/current/pressed state. */ state(node: Element): string;
  /** Canonical field validation facts. */ facts(node: Element): Record<string, unknown>;
  /** Scope all semantic caches to one synchronous read. */ begin(): void;
  /** Clear caches even if classification fails. */ finish(): void;
}
/** Only this private content world can observe the compile-time semantic entry. */
interface SemanticWorld {
  /** Frozen canonical semantic methods. */ __acDiagnosticSemantics: Semantics;
}
const semantics = (window as unknown as SemanticWorld).__acDiagnosticSemantics;
/** Missing semantic support is explicit; it never chooses a different or synthetic input path. */
export function beginDebugSemantics(): void {
  if (!semantics) throw Error('Diagnostic control semantics unavailable');
  semantics.begin();
}
/** A completed or failed read cannot retain stale DOM measurements. */
export function finishDebugSemantics(): void {
  semantics?.finish();
}
/** Classification stays identical to the existing analyzer, including hidden proxies and site widgets. */
export function debugInteractive(node: Element): boolean {
  return semantics.type(node) !== null;
}
/** The canonical analyzer names icon-only controls and the record a button selects. */
export function debugLabel(node: Element): string {
  return semantics.name(node, semantics.type(node) ?? '').slice(0, C.MAX_LABEL);
}
/** Native and custom proxies retain their actual interactive kind. */
export function debugControlRole(node: Element): string {
  const kind = semantics.type(node) ?? node.localName;
  const roles: Record<string, string> = {
    input: 'textbox',
    textarea: 'textbox',
    editable: 'textbox',
    select: 'combobox',
  };
  return node.getAttribute('role') || roles[kind] || kind;
}
/** Field facts and radio position come from the underlying control while actions retain the visible node. */
function canonicalFacts(node: Element): Record<string, unknown> {
  const facts = semantics.facts(semantics.control(node)),
    checked = semantics.checked(node);
  if (checked !== undefined) facts.checked = checked;
  for (const [key, value] of [
    ['choice', semantics.choice(node)],
    ['state', semantics.state(node)],
  ])
    if (value) facts[key] = value;
  return facts;
}
/** All author-controlled values remain bounded before transport. */
function text(value: string): string {
  return value.replace(/\s+/g, ' ').trim().slice(0, C.MAX_LABEL);
}
/** Form facts retain useful state and options while redacting password contents at the source. */
export function debugFieldFacts(node: Element): Record<string, unknown> {
  const facts: Record<string, unknown> = canonicalFacts(node);
  node = semantics.control(node);
  if (node instanceof HTMLInputElement || node instanceof HTMLTextAreaElement) {
    facts.value = node.type === 'password' ? '••••' : text(node.value);
    if (node.placeholder) facts.placeholder = text(node.placeholder);
    if (node.readOnly) facts.readonly = true;
  }
  if (node instanceof HTMLInputElement && ['checkbox', 'radio'].includes(node.type)) facts.checked = node.checked;
  if (node instanceof HTMLSelectElement) {
    facts.value = text(node.value);
    facts.options = [...node.options].slice(0, C.MAX_OPTIONS).map((option) => text(option.text));
    facts.optionsOmitted = Math.max(0, node.options.length - C.MAX_OPTIONS);
  }
  if (('disabled' in node && node.disabled) || node.getAttribute('aria-disabled') === 'true') facts.disabled = true;
  if ('required' in node && node.required) facts.required = true;
  if (node.hasAttribute('aria-checked')) facts.checked = node.getAttribute('aria-checked');
  return facts;
}
