/**
 * Unit tests for a playbook's placeholders: which it uses, which a run must
 * supply, and turning recorded values into named placeholders with defaults.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  HAS_PLACEHOLDER,
  variablesOf,
  namesOf,
  missingVariables,
  templateValues,
} from '../../../../src/modules/playbooks/variables.ts';

/** A playbook of these steps, templated. */
const templated = (steps, prompt?) => templateValues({ steps, defaults: {}, prompt });

describe('placeholders', () => {
  it('spots a placeholder with or without filters', () => {
    assert.ok(HAS_PLACEHOLDER.test('a {{x}} b'));
    assert.ok(HAS_PLACEHOLDER.test('{{x|first|date:MM}}'));
    assert.ok(!HAS_PLACEHOLDER.test('{x}'));
  });

  it('lists each placeholder once, in order of appearance', () => {
    assert.deepEqual(variablesOf([{ url: '{{b}}' }, { text: '{{a|upper}} {{b}}' }]), ['b', 'a']);
  });

  it('reads a version-2 workflow’s names through the workflow engine', () => {
    const pb = { schemaVersion: 2, steps: [{ action: 'navigate', url: 'https://a.test/{{q}}' }] };
    assert.deepEqual(namesOf(pb), ['q']);
  });

  describe('missingVariables', () => {
    const pb = {
      steps: [{ text: '{{a}} {{b}} {{c}} {{d}} {{e}}' }],
      defaults: { b: 'B' },
      variables: { c: { default: 'C' }, d: { secret: true, default: 'D' } },
    };

    it('asks for what has no value passed and no default', () => {
      assert.deepEqual(missingVariables(pb), ['a', 'd', 'e']);
    });

    it('never counts a secret’s default as a value', () => {
      assert.ok(missingVariables(pb, { a: 'x', e: 'y' }).includes('d'));
    });

    it('is satisfied by passed values, including zero', () => {
      assert.deepEqual(missingVariables(pb, { a: 0, d: 'x', e: '' }), []);
    });
  });
});

describe('templateValues', () => {
  it('turns a typed value into a placeholder named after the field, keeping the value as its default', () => {
    const pb = templated([{ action: 'type', text: 'ada@x.test', el: { name: 'email' } }]);
    assert.equal(pb.steps[0].text, '{{email}}');
    assert.deepEqual(pb.defaults, { email: 'ada@x.test' });
    assert.deepEqual(pb.labels, []);
  });

  it('templates a picked option, named from the select', () => {
    const pb = templated([{ action: 'select_option', option: 'Blue', el: { ariaLabel: 'Favourite colour' } }]);
    assert.equal(pb.steps[0].option, '{{favouriteColour}}');
  });

  it('leaves clicks, navigations and values that already hold a placeholder alone', () => {
    const steps = [
      { action: 'click', el: { text: 'Go' } },
      { action: 'navigate', url: 'https://a.test/' },
      { action: 'type', text: '{{password}}', el: { name: 'pw' } },
      { action: 'type', text: '', el: { name: 'x' } },
    ];
    const pb = templated(structuredClone(steps));
    assert.deepEqual(pb.steps, steps);
    assert.deepEqual(pb.defaults, {});
  });

  it('prefixes a name that is not an identifier with the element’s kind', () => {
    const pb = templated([{ action: 'type', text: 'x', el: { type: 'input', name: '2024 total' } }]);
    assert.equal(pb.steps[0].text, '{{input2024Total}}');
  });

  it('names an unlabelled field after what it is and where it was', () => {
    const pb = templated([
      { action: 'click', el: {} },
      { action: 'type', text: 'x', el: { tag: 'textarea' } },
    ]);
    assert.equal(pb.steps[1].text, '{{textarea2}}');
    assert.equal(templated([{ action: 'type', text: 'x' }]).steps[0].text, '{{field1}}');
  });

  it('gives a second field with the same name a numbered one', () => {
    const pb = templated([
      { action: 'type', text: 'a', el: { name: 'q' } },
      { action: 'type', text: 'b', el: { name: 'q' } },
      { action: 'type', text: 'c', el: { name: 'q' } },
    ]);
    assert.deepEqual(
      pb.steps.map((s) => s.text),
      ['{{q}}', '{{q2}}', '{{q3}}'],
    );
  });

  it('keeps one name for one field typed into twice with the same value', () => {
    const pb = templated([
      { action: 'type', text: 'a', el: { name: 'q' } },
      { action: 'type', text: 'a', el: { name: 'q' } },
    ]);
    assert.deepEqual(
      pb.steps.map((s) => s.text),
      ['{{q}}', '{{q}}'],
    );
  });

  it('carries typed values into the healing prompt, but not short ones', () => {
    const pb = templated(
      [
        { action: 'type', text: 'Lovelace', el: { name: 'last' } },
        { action: 'type', text: 'NY', el: { name: 'state' } },
      ],
      'Sign up Lovelace in NY',
    );
    assert.equal(pb.prompt, 'Sign up {{last}} in NY');
  });
});
