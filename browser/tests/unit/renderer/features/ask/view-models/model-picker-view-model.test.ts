/**
 * Unit tests for the model picker and its search: every word must match,
 * matches are marked, OpenRouter's models are grouped by vendor, the keys
 * move and choose, and an id the list lacks can be used as typed.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { ModelPickerViewModel } from '../../../../../../src/renderer/features/ask/view-models/model-picker-view-model.ts';
import {
  groupHeading,
  isGrouped,
  markParts,
  modelCount,
  searchWords,
  splitModelLabel,
  triggerText,
} from '../../../../../../src/renderer/features/ask/model/model-search.ts';

/** OpenRouter's models from two vendors. */
const MODELS = [
  { id: 'anthropic/claude-sonnet-5', label: 'Anthropic: Claude Sonnet 5' },
  { id: 'anthropic/claude-opus-5.5', label: 'Anthropic: Claude Opus 5.5' },
  { id: 'x-ai/grok-4.7', label: 'xAI: Grok 4.7' },
];

/** A picker on MODELS with Claude Sonnet 5 chosen, its list open. */
function opened() {
  const picker = new ModelPickerViewModel();
  picker.setModels(MODELS, 'anthropic/claude-sonnet-5');
  picker.openList();
  return picker;
}

describe('ModelPickerViewModel', () => {
  it('matches every word typed against name and id, then offers the query as a custom id', () => {
    const picker = opened();
    picker.search('claude son');
    assert.deepEqual(
      picker.state.shown.map((o) => o.id),
      ['anthropic/claude-sonnet-5', 'claude son'],
    );
  });

  it('marks the matched words in a name', () => {
    const name = splitModelLabel('Anthropic: Claude Sonnet 5').name;
    const marked = markParts(name, searchWords('claude son')).filter((p) => p.mark);
    assert.deepEqual(
      marked.map((p) => p.text),
      ['Claude', 'Son'],
    );
  });

  it('keeps a hostile name as plain text parts', () => {
    const label = '<img src=x onerror=alert(1)>';
    assert.deepEqual(markParts(label, []), [{ text: label, mark: false }]);
  });

  it('groups models by vendor when there is more than one, and says how many there are', () => {
    const picker = opened();
    const { shown } = picker.state;
    const grouped = isGrouped(shown);
    assert.deepEqual(shown.map((_, i) => groupHeading(shown, i, grouped)).filter(Boolean), ['Anthropic', 'xAI']);
    assert.equal(modelCount(shown), '3 models');
  });

  it('opens on the chosen model, and on the first match once something is typed', () => {
    const picker = new ModelPickerViewModel();
    picker.setModels(MODELS, 'x-ai/grok-4.7');
    picker.openList();
    assert.equal(picker.state.active, 2);
    picker.search('claude');
    assert.equal(picker.state.active, 0);
  });

  it('moves with the arrow keys and chooses with Enter, wrapping at the ends', () => {
    const picker = opened();
    assert.equal(picker.key('ArrowUp'), true);
    picker.key('Enter');
    assert.equal(triggerText(picker.state.models, picker.state.value).name, 'Grok 4.7');
    assert.equal(picker.state.open, false);
  });

  it('chooses the option under the pointer with Enter', () => {
    const picker = opened();
    picker.hover(1);
    picker.key('Enter');
    assert.equal(picker.state.value, 'anthropic/claude-opus-5.5');
  });

  it('uses an id the list lacks, as typed', () => {
    const picker = opened();
    picker.search('meta-llama/llama-5');
    picker.key('Enter');
    assert.equal(picker.state.value, 'meta-llama/llama-5');
    assert.deepEqual(triggerText(picker.state.models, picker.state.value), {
      name: 'meta-llama/llama-5',
      detail: 'Custom model id',
    });
  });

  it('closes on Escape without changing the model', () => {
    const picker = opened();
    picker.search('grok');
    picker.key('Escape');
    assert.equal(picker.state.open, false);
    assert.equal(picker.state.value, 'anthropic/claude-sonnet-5');
  });

  it('leaves other keys to the search box', () => {
    assert.equal(opened().key('a'), false);
  });

  it('does nothing on the arrows or Enter with nothing listed', () => {
    const picker = new ModelPickerViewModel();
    picker.openList();
    picker.key('ArrowDown');
    picker.key('Enter');
    assert.deepEqual([picker.state.active, picker.state.value, picker.state.open], [0, '', true]);
  });

  it('toggles the list from its button', () => {
    const picker = new ModelPickerViewModel();
    picker.toggle();
    assert.equal(picker.state.open, true);
    picker.toggle();
    assert.equal(picker.state.open, false);
  });

  it('names an unchosen picker and shows a vendorless model’s id underneath', () => {
    assert.equal(triggerText([], '').name, 'Choose a model');
    assert.deepEqual(triggerText([{ id: 'gpt-4.1', label: 'GPT-4.1' }], 'gpt-4.1'), {
      name: 'GPT-4.1',
      detail: 'gpt-4.1',
    });
  });
});
