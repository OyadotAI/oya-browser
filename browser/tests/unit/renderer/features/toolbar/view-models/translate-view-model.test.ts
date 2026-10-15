/**
 * Unit tests for the translate chip: what it says in each state, what a click
 * does, and what a screen reader hears (only when a translation starts, ends,
 * fails or is undone on the same tab).
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  TranslateViewModel,
  announcement,
  chipLook,
} from '../../../../../../src/renderer/features/toolbar/view-models/translate-view-model.ts';
import { fakeBridge } from '../../../support/bridge.ts';
import type { TranslateFailure } from '../../../../../../src/shared/ipc.ts';

/** A translation of a Japanese page for an English reader. */
const ja = (state: string, error?: TranslateFailure) =>
  ({ state, lang: 'ja', target: 'en-US', ...(error ? { error } : {}) }) as any;

/** One active tab on `url` with `translation`. */
const tabs = (translation: any, url = 'https://a.jp/', id = 1) => [{ id, url, active: true, translation }];

/** A failure with `code`, as the server says it. */
const failed = (code: string, message = 'Server says why.') => ja('failed', { code, message });

describe('chipLook', () => {
  it('is hidden with nothing to offer', () => {
    assert.equal(chipLook(undefined), null);
  });

  it('offers, translates, and shows how it ended', () => {
    assert.equal(chipLook(ja('offer'))?.text, 'Translate from Japanese');
    assert.equal(chipLook(ja('offer'))?.title, "Translate this page into English with your project's AI model");
    assert.equal(chipLook(ja('translating'))?.text, 'Translating…');
    assert.equal(chipLook(ja('translated'))?.text, 'Translated from Japanese · Show original');
    assert.equal(chipLook({ ...ja('translated'), lang: '' })?.text, 'Translated to English · Show original');
    assert.equal(chipLook(ja('partial'))?.text, 'Translated the first part · Show original');
    assert.equal(chipLook(ja('translated'))?.tone, 'done');
  });

  it('turns each failure code into its own words and click', () => {
    const cases: Array<[string, string, string]> = [
      ['signed_out', 'Sign in to translate', 'ask'],
      ['llm_unconfigured', 'Connect a model to translate', 'models'],
      ['llm_bring_your_own', 'Connect a model to translate', 'models'],
      ['llm_not_allowed', 'Model not allowed', 'models'],
      ['llm_rejected', 'Model key refused', 'models'],
      ['llm_payment_required', 'Provider needs payment', 'translate'],
      ['llm_unavailable', "Model didn't answer", 'translate'],
      ['llm_quota', 'Hourly limit reached', 'translate'],
      ['rate_limited', 'Too many requests', 'translate'],
      ['claim_required', 'Free steps used up', 'claim'],
      ['offline', "Couldn't reach Oya", 'translate'],
      ['timeout', 'Translation timed out', 'translate'],
      ['nothing_to_translate', 'Nothing to translate', 'none'],
      ['translate_malformed', "Couldn't translate", 'translate'],
      ['license_required', "Couldn't translate", 'translate'],
    ];
    for (const [code, text, action] of cases)
      assert.deepEqual([chipLook(failed(code))?.text, chipLook(failed(code))?.action], [text, action], code);
  });

  it('uses the server’s message as the reason where it is written for a person, and its own words elsewhere', () => {
    assert.equal(chipLook(failed('llm_not_allowed'))?.title, 'Server says why.');
    assert.equal(chipLook(failed('something_new'))?.title, 'Server says why.');
    assert.match(chipLook(failed('llm_quota'))?.title ?? '', /Translation and Assistant share/);
  });

  it('shows setup and an empty page plainly, and real failures in the danger colour', () => {
    assert.equal(chipLook(failed('signed_out'))?.tone, 'idle');
    assert.equal(chipLook(failed('nothing_to_translate'))?.tone, 'idle');
    assert.equal(chipLook(failed('offline'))?.tone, 'failed');
  });
});

describe('announcement', () => {
  it('says nothing when an offer appears', () => {
    assert.equal(announcement(undefined, ja('offer')), '');
  });

  it('says when a translation starts, ends, ends in part, fails or is undone', () => {
    assert.equal(announcement(ja('offer'), ja('translating')), 'Translating from Japanese');
    assert.equal(announcement(ja('translating'), ja('translated')), 'Translated from Japanese');
    assert.equal(announcement(ja('translating'), ja('partial')), 'Translated the first part of this page');
    assert.equal(announcement(ja('translated'), ja('offer')), 'Showing the original page');
    assert.match(announcement(ja('translating'), failed('offline')), /can't be reached/);
  });
});

describe('TranslateViewModel', () => {
  it('follows the active tab, and hides on a page that is not on the web', () => {
    const fake = fakeBridge();
    const vm = new TranslateViewModel(fake.bridge);
    fake.emit('onTabsUpdated', tabs(ja('offer')));
    assert.equal(vm.state.chip?.text, 'Translate from Japanese');
    fake.emit('onTabsUpdated', tabs(ja('offer'), 'file:///x'));
    assert.equal(vm.state.chip, null);
    vm.dispose();
  });

  it('announces a change on the same tab, not a switch to another tab', () => {
    const fake = fakeBridge();
    const vm = new TranslateViewModel(fake.bridge);
    fake.emit('onTabsUpdated', tabs(ja('offer')));
    fake.emit('onTabsUpdated', tabs(ja('translating')));
    assert.equal(vm.state.said, 'Translating from Japanese');
    fake.emit('onTabsUpdated', tabs(ja('translated'), 'https://b.jp/', 2));
    assert.equal(vm.state.said, 'Translating from Japanese');
    vm.dispose();
  });

  it('translates on an offer, shows the original once translated, and sends setup clicks to the host', async () => {
    const fake = fakeBridge();
    const opened: string[] = [];
    const host = {
      openAsk: () => opened.push('ask'),
      openModels: () => opened.push('models'),
      openClaim: () => opened.push('claim'),
    };
    const vm = new TranslateViewModel(fake.bridge, host);
    for (const t of [
      ja('offer'),
      ja('translated'),
      failed('signed_out'),
      failed('llm_rejected'),
      failed('claim_required'),
      failed('nothing_to_translate'),
    ]) {
      fake.emit('onTabsUpdated', tabs(t));
      vm.activate();
    }
    await new Promise(setImmediate);
    assert.deepEqual(fake.called('translatePage'), [[1]]);
    assert.deepEqual(fake.called('showOriginal'), [[1]]);
    assert.deepEqual(opened, ['ask', 'models', 'claim']);
    vm.dispose();
  });
});
