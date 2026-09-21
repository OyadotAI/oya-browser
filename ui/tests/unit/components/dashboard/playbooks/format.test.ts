/**
 * Unit tests for the Playbooks tab's display rules: step lines, names, run
 * status text and the frame's age.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import {
  describeStep,
  frameAge,
  isEnded,
  isPlaybookName,
  replyPlaceholder,
  statusClass,
  statusLabel,
  withScheme,
} from '@/components/dashboard/playbooks/format';

describe('describeStep', () => {
  it('reads each known action in its own words', () => {
    expect(describeStep({ action: 'navigate', url: 'https://a.com' })).toBe('navigate https://a.com');
    expect(describeStep({ action: 'type', text: 'Ada', el: { name: 'first' } })).toBe('type first ← Ada');
    expect(describeStep({ action: 'select_option', option: 'NY', el: { domId: 'state' } })).toBe('select state ← NY');
    expect(describeStep({ action: 'press_key', key: 'Enter' })).toBe('key Enter');
  });

  it('falls back to the action and label for any other action', () => {
    expect(describeStep({ action: 'click', el: { text: 'Submit' } })).toBe('click Submit');
  });

  it('labels an element by text, then name, id, test id and tag', () => {
    expect(describeStep({ action: 'click', el: { name: 'n', domId: 'd', tag: 'button' } })).toBe('click n');
    expect(describeStep({ action: 'click', el: { testId: 't', tag: 'a' } })).toBe('click t');
    expect(describeStep({ action: 'click', el: { tag: 'a' } })).toBe('click a');
    expect(describeStep({ action: 'click' })).toBe('click ');
  });

  it('does not treat inherited object keys as actions', () => {
    expect(describeStep({ action: 'toString' })).toBe('toString ');
  });
});

describe('names and addresses', () => {
  it('accepts 1-64 letters, digits, _ or -', () => {
    expect(isPlaybookName('portal-login_2')).toBe(true);
    expect(isPlaybookName('')).toBe(false);
    expect(isPlaybookName('has space')).toBe(false);
    expect(isPlaybookName('x'.repeat(65))).toBe(false);
  });

  it('adds https:// only to an address without a scheme', () => {
    expect(withScheme('example.com')).toBe('https://example.com');
    expect(withScheme('HTTP://a.com')).toBe('HTTP://a.com');
  });
});

describe('run status', () => {
  it('ends on success or failure only', () => {
    expect(isEnded('succeeded')).toBe(true);
    expect(isEnded('failed')).toBe(true);
    expect(isEnded('running')).toBe(false);
    expect(isEnded(undefined)).toBe(false);
  });

  it('colours and names each status', () => {
    expect(statusClass('failed')).toBe('text-red');
    expect(statusClass('needs_attention')).toBe('text-yellow');
    expect(statusClass('running')).toBe('text-accent');
    expect(statusLabel('needs_attention')).toBe('needs a person');
    expect(statusLabel('succeeded')).toBe('succeeded');
  });

  it('hints an answer for an agent question, a code for MFA, and "done" otherwise', () => {
    expect(replyPlaceholder('agent')).toBe('Your answer');
    expect(replyPlaceholder('mfa')).toMatch(/Paste the code/);
    expect(replyPlaceholder('captcha')).toBe('done');
  });
});

describe('frameAge', () => {
  afterEach(() => vi.useRealTimers());

  it('is null before the first frame and the elapsed ms after', () => {
    vi.useFakeTimers({ now: 5000 });
    expect(frameAge(null)).toBeNull();
    expect(frameAge(4200)).toBe(800);
  });
});
