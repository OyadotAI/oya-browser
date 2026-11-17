/**
 * Unit tests for the playbook editor's definition helpers (editor/definition.ts):
 * parsing the text, finding the variables the steps use, and setting a default
 * or the secret flag.
 */
import { describe, it, expect } from 'vitest';
import {
  parse,
  stepSummary,
  withStep,
  withoutStep,
  toText,
  variableRows,
  variablesUsed,
  withDefault,
  withSecret,
} from '@/components/dashboard/playbooks/editor/definition';
import { canConfirm } from '@/components/dashboard/playbooks/editor/use-step-panel';

const DEF = {
  steps: [
    { action: 'navigate', url: 'https://a.test/{{path}}' },
    { action: 'type', text: '{{ email | lower }}' },
    { action: 'type', text: '{{path}} again' },
  ],
  defaults: { path: 'signup' },
  secrets: ['pw'],
};

describe('definition helpers', () => {
  it('parses a definition and says why other text is not one', () => {
    expect(parse(toText(DEF)).definition).toEqual(DEF);
    expect(parse('{').error).toMatch(/^Not valid JSON/);
    expect(parse('[]').error).toBe('The definition needs a steps list');
    expect(parse('{"steps": 1}').error).toBe('The definition needs a steps list');
  });

  it('finds each variable the steps use once, filters and spaces allowed', () => {
    expect(variablesUsed(DEF)).toEqual(['path', 'email']);
    expect(variablesUsed({})).toEqual([]);
  });

  it('lists each variable with its default and secret flag', () => {
    expect(variableRows(withSecret(DEF, 'email', true))).toEqual([
      { name: 'path', value: 'signup', secret: false },
      { name: 'email', value: '', secret: true },
    ]);
  });

  it('sets and clears a default, and marks a variable secret and back', () => {
    expect(withDefault(DEF, 'email', 'a@b').defaults).toEqual({ path: 'signup', email: 'a@b' });
    expect(withDefault(DEF, 'path', '').defaults).toEqual({});
    expect(withSecret(withSecret(DEF, 'email', true), 'email', false).secrets).toEqual(['pw']);
  });

  it('edits a workflow’s variables where replay reads them, not the classic defaults', () => {
    const wf = {
      schemaVersion: 2,
      steps: [{ action: 'type', text: '{{email}}' }],
      variables: { pw: { secret: true } },
    };
    expect(variableRows(wf)).toEqual([
      { name: 'email', value: '', secret: false },
      { name: 'pw', value: '', secret: true },
    ]);
    const edited = withSecret(withDefault(wf, 'email', 'a@b'), 'pw', false);
    expect(edited.variables).toEqual({ email: { default: 'a@b' }, pw: { secret: false } });
    expect(edited).not.toHaveProperty('defaults');
  });
});

describe('steps in the visual view', () => {
  it('summarises each step by what it does', () => {
    expect(stepSummary({ action: 'navigate', url: 'https://a.test' })).toBe('https://a.test');
    expect(stepSummary({ action: 'type', text: '{{email}}' })).toBe('{{email}}');
    expect(stepSummary({ action: 'press_key', key: 'Enter' })).toBe('Enter');
    expect(stepSummary({ action: 'scroll', direction: 'down', amount: 3 })).toBe('down 3');
    expect(stepSummary({ action: 'click', el: { text: 'Go', ariaLabel: 'Submit' } })).toBe('Submit');
    expect(stepSummary({ action: 'assert_text', expected: 'Approved' })).toBe('Approved');
    expect(stepSummary({ action: 'click' })).toBe('');
  });

  it('an edit replaces only the edited fields, keeping key order, other steps and top-level fields', () => {
    const d = { steps: [{ action: 'navigate', url: 'a', start: true }, { action: 'click' }], defaults: { x: '1' } };
    const edited = withStep(d, 0, { url: 'b' });
    expect(edited.steps).toEqual([{ action: 'navigate', url: 'b', start: true }, { action: 'click' }]);
    expect(Object.keys(edited.steps![0] as object)).toEqual(['action', 'url', 'start']);
    expect(edited.defaults).toEqual({ x: '1' });
  });

  it('deleting removes just that step', () => {
    const d = { steps: [{ action: 'a' }, { action: 'b' }, { action: 'c' }] };
    expect(withoutStep(d, 1).steps).toEqual([{ action: 'a' }, { action: 'c' }]);
  });
});

describe('confirming a step', () => {
  it('needs a url and a numeric amount where the step has them', () => {
    expect(canConfirm({ url: 'https://a.test', amount: '3' })).toBe(true);
    expect(canConfirm({ url: ' ' })).toBe(false);
    expect(canConfirm({ amount: '' })).toBe(false);
    expect(canConfirm({ amount: 'x' })).toBe(false);
    expect(canConfirm({ key: 'Enter' })).toBe(true);
  });
});
