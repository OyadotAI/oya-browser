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

  it('templates keyboard_type text like typed text', () => {
    const pb = templated([{ action: 'keyboard_type', text: 'red shoes' }]);
    assert.equal(pb.steps[0].text, '{{field1}}');
    assert.deepEqual(pb.defaults, { field1: 'red shoes' });
  });

  it('puts the variable where a typed value echoes in an address, in every spelling', () => {
    const pb = templated([
      { action: 'type', text: 'red shoes', el: { name: 'q' } },
      { action: 'navigate', url: 'https://a.test/s?k=red+shoes&x=1' },
      { action: 'navigate', url: 'https://a.test/search/red%20shoes' },
    ]);
    assert.equal(pb.steps[1].url, 'https://a.test/s?k={{q|url}}&x=1');
    assert.equal(pb.steps[2].url, 'https://a.test/search/{{q|url}}');
  });

  it('leaves a value that is only part of a path segment, or the host, alone', () => {
    const pb = templated([
      { action: 'type', text: 'new', el: { name: 'q' } },
      { action: 'type', text: 'a.test', el: { name: 'site' } },
      { action: 'navigate', url: 'https://a.test/news/?tab=newest' },
    ]);
    assert.equal(pb.steps[2].url, 'https://a.test/news/?tab=newest');
  });

  it('makes a click on the typed value a data-driven click', () => {
    const pb = templated([
      { action: 'type', text: 'Red Shoes', el: { name: 'q' } },
      { action: 'click', el: { text: 'red shoes', tag: 'a' } },
      { action: 'click', el: { text: 'Red Shoes Deluxe', tag: 'a' } },
    ]);
    assert.equal(pb.steps[1].el.text, '{{q}}');
    assert.equal(pb.steps[2].el.text, '{{q}} Deluxe');
  });

  it('makes the option picked right after typing a data-driven click, keeping its other words', () => {
    const pb = templated([
      { action: 'type', text: '70450', el: { name: 'cpt' } },
      { action: 'click', el: { tag: 'li', text: '70450 - CT head/brain without contrast' } },
    ]);
    assert.equal(pb.steps[1].el.text, '{{cpt}} - CT head/brain without contrast');
  });

  it('leaves a click far from where the value was typed alone', () => {
    const pb = templated([
      { action: 'select_option', option: 'Yes', el: { name: 'performed' } },
      { action: 'press_key', key: 'Tab' },
      { action: 'press_key', key: 'Tab' },
      { action: 'press_key', key: 'Tab' },
      { action: 'click', el: { tag: 'button', text: 'Yes, continue' } },
    ]);
    assert.equal(pb.steps[4].el.text, 'Yes, continue');
  });

  it('keeps an existing variable when a healed playbook is templated again', () => {
    const pb = {
      name: 'x',
      prompt: '',
      defaults: { q: 'shoes' },
      steps: [
        { action: 'type', text: '{{q}}', el: { name: 'q' } },
        { action: 'type', text: 'boots', el: { name: 'q' } },
      ],
    };
    templateValues(pb);
    assert.deepEqual(pb.defaults, { q: 'shoes', q2: 'boots' });
  });

  it('makes a choice the prompt names a variable, named from the words before it', () => {
    const pb = templated(
      [{ action: 'click', el: { tag: 'button', text: 'Diagnostic Imaging' } }],
      'CPT 70450, service category Diagnostic Imaging, quantity 1',
    );
    assert.equal(pb.steps[0].el.text, '{{serviceCategory}}');
    assert.deepEqual(pb.defaults, { serviceCategory: 'Diagnostic Imaging' });
    assert.equal(pb.prompt, 'CPT 70450, service category {{serviceCategory}}, quantity 1');
  });

  it('leaves links, one-word labels and labels the prompt does not name alone', () => {
    const steps = [
      { action: 'click', el: { tag: 'a', href: '/auth', text: 'Authorizations & Referrals' } },
      { action: 'click', el: { tag: 'button', text: 'Submit' } },
      { action: 'click', el: { tag: 'button', text: 'Save draft' } },
    ];
    const pb = templated(structuredClone(steps), 'Open Authorizations & Referrals, fill it in and Submit it.');
    assert.deepEqual(pb.steps, steps);
    assert.deepEqual(pb.defaults, {});
  });

  it('gives a choice clicked twice the same variable', () => {
    const pick = { action: 'click', el: { tag: 'button', text: 'Diagnostic Imaging' } };
    const pb = templated([structuredClone(pick), structuredClone(pick)], 'service category Diagnostic Imaging');
    assert.deepEqual(
      pb.steps.map((s) => s.el.text),
      ['{{serviceCategory}}', '{{serviceCategory}}'],
    );
  });

  it('makes text the agent wrote itself an answer the model writes fresh, with no default', () => {
    const pb = templated(
      [
        {
          action: 'type',
          text: 'Patient has had headaches for three months.',
          el: { tag: 'textarea', text: 'Clinical notes' },
        },
      ],
      'Request a CT for Maria Lindqvist',
    );
    assert.deepEqual(pb.steps[0].answer, {
      key: 'clinicalNotes',
      question: 'Clinical notes',
      example: 'Patient has had headaches for three months.',
    });
    assert.equal(pb.steps[0].text, undefined);
    assert.deepEqual(pb.defaults, {});
  });

  it('keeps text the prompt gave as a variable, however long', () => {
    const note = 'Leave it with the front desk please';
    const pb = templated(
      [{ action: 'type', text: note, el: { tag: 'textarea', name: 'notes' } }],
      `Order pizza. Notes: ${note}`,
    );
    assert.equal(pb.steps[0].text, '{{notes}}');
    assert.equal(pb.steps[0].answer, undefined);
  });

  it('never makes an answer of a recording with no prompt to answer from', () => {
    const pb = templated([
      { action: 'type', text: 'some words typed by a person', el: { tag: 'textarea', name: 'c' } },
    ]);
    assert.equal(pb.steps[0].text, '{{c}}');
  });

  it('names a radio or checkbox the prompt picks after its group, even a one-word one', () => {
    const radio = (name, text) => ({ action: 'click', el: { tag: 'input', type: 'radio', name, text } });
    const pb = templated(
      [radio('size', 'Medium'), radio('conservative', 'Yes'), radio('surgical', 'Yes'), radio('performed', 'No')],
      'Order a medium pizza. Conservative care Yes, surgical planning Yes, performed No.',
    );
    assert.deepEqual(
      pb.steps.map((s) => s.el.text),
      ['{{size}}', '{{conservative}}', '{{surgical}}', '{{performed}}'],
    );
    assert.deepEqual(pb.defaults, { size: 'Medium', conservative: 'Yes', surgical: 'Yes', performed: 'No' });
  });

  it('leaves a radio the prompt does not name alone', () => {
    const pb = templated(
      [{ action: 'click', el: { tag: 'input', type: 'radio', name: 'size', text: 'Large' } }],
      'Order a pizza',
    );
    assert.equal(pb.steps[0].el.text, 'Large');
  });

  it('keeps a prompt value typed twice a variable both times', () => {
    const address = '1 Navy Yard, Arlington VA';
    const type = { action: 'type', text: address, el: { tag: 'textarea', domId: 'currentAddress' } };
    const pb = templated([structuredClone(type), structuredClone(type)], `Current address ${address}`);
    assert.deepEqual(
      pb.steps.map((s) => s.text),
      ['{{currentaddress}}', '{{currentaddress}}'],
    );
  });
});
