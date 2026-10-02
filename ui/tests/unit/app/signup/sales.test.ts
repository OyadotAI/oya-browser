/**
 * Unit tests for the enterprise path's rules: what the answers need, the
 * note the founders get, the prefilled Calendly link, and the booked signal.
 */
import { describe, it, expect } from 'vitest';
import { EMPTY_DETAILS, bookingNotes, bookingUrl, isBooked, salesProblem, toggled } from '@/app/signup/sales';

/** Answers good enough to book with. */
const filled = { ...EMPTY_DETAILS, firstName: 'Ann', lastName: 'Lee', email: ' ann@acme.test ', company: 'Acme' };

describe('salesProblem', () => {
  it('asks for a first name, a work email and a company, in that order', () => {
    expect(salesProblem(EMPTY_DETAILS)).toBe('First name is required');
    expect(salesProblem({ ...filled, email: ' ' })).toBe('Work email is required');
    expect(salesProblem({ ...filled, company: '' })).toBe('Company is required');
    expect(salesProblem(filled)).toBe('');
  });
});

describe('toggled', () => {
  it('adds a missing choice and removes a present one', () => {
    expect(toggled(['a'], 'b')).toEqual(['a', 'b']);
    expect(toggled(['a', 'b'], 'a')).toEqual(['b']);
  });
});

describe('bookingNotes', () => {
  it('lists the answers by name and leaves out the unanswered ones', () => {
    const notes = bookingNotes({ ...filled, portals: ['EHRs', 'Registries'], deploy: 'self', notes: ' nightly ' });
    expect(notes).toBe(
      'Company: Acme\nRole: Engineering leader\nNeeds to reach: EHRs, Registries\nRuns on: Our cloud\n' +
        'Runs / month: Not sure yet\nNotes: nightly',
    );
  });
});

describe('bookingUrl', () => {
  it('prefills the founders call with the name, trimmed email and notes', () => {
    const url = new URL(bookingUrl(filled, 'oyabrowser.com'));
    expect(url.origin + url.pathname).toBe('https://calendly.com/d/dvrm-r65-kkx/oya-founder-call');
    expect(url.searchParams.get('name')).toBe('Ann Lee');
    expect(url.searchParams.get('email')).toBe('ann@acme.test');
    expect(url.searchParams.get('a1')).toBe(bookingNotes(filled));
    expect(url.searchParams.get('embed_domain')).toBe('oyabrowser.com');
  });
});

describe('isBooked', () => {
  it('trusts only a scheduled event from Calendly itself', () => {
    const data = { event: 'calendly.event_scheduled' };
    expect(isBooked(new MessageEvent('message', { origin: 'https://calendly.com', data }))).toBe(true);
    expect(isBooked(new MessageEvent('message', { origin: 'https://evil.test', data }))).toBe(false);
    const viewed = { event: 'calendly.date_and_time_selected' };
    expect(isBooked(new MessageEvent('message', { origin: 'https://calendly.com', data: viewed }))).toBe(false);
    expect(isBooked(new MessageEvent('message', { origin: 'https://calendly.com', data: null }))).toBe(false);
  });
});
