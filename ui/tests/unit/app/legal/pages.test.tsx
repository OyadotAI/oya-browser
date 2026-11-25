/**
 * Unit tests for the Privacy Policy and Terms of Service, through what a visitor
 * sees: who they are with, a contents list whose links reach their sections,
 * where to write, and both pages linked from the footer and from sign-up.
 */
import { describe, it, expect, afterEach } from 'vitest';
import { cleanup, render, screen, within } from '@testing-library/react';
import PrivacyPolicy from '@/app/privacy/page';
import TermsOfService from '@/app/terms/page';
import { SignupForm } from '@/app/signup/signup-form';
import { COMPANY, LEGAL_EMAIL } from '@/app/_legal/content';

/** Both pages, by name. */
const PAGES = [
  ['Privacy Policy', PrivacyPolicy],
  ['Terms of Service', TermsOfService],
] as const;

describe.each(PAGES)('%s', (title, Page) => {
  afterEach(() => cleanup());

  it('names the company the visitor deals with', () => {
    render(<Page />);
    expect(screen.getByRole('heading', { level: 1, name: title })).toBeTruthy();
    expect(screen.getAllByText(new RegExp(COMPANY)).length).toBeGreaterThan(0);
  });

  it('links every contents entry to a section on the page', () => {
    const { container } = render(<Page />);
    const contents = screen.getByRole('navigation', { name: 'Contents' });
    for (const link of within(contents).getAllByRole('link')) {
      const id = link.getAttribute('href')!.slice(1);
      expect(container.querySelector(`section[id="${id}"]`), id).toBeTruthy();
    }
  });

  it('says where to write', () => {
    render(<Page />);
    expect(screen.getAllByRole('link', { name: LEGAL_EMAIL })[0].getAttribute('href')).toBe(`mailto:${LEGAL_EMAIL}`);
  });

  it('is linked from the site footer', () => {
    render(<Page />);
    const footer = screen.getByRole('contentinfo', { name: 'Site footer' });
    const name = title === 'Privacy Policy' ? 'Privacy' : 'Terms';
    expect(within(footer).getByRole('link', { name }).getAttribute('href')).toBe(
      title === 'Privacy Policy' ? '/privacy' : '/terms',
    );
  });
});

/** A captcha with no site key: off, as in development. */
const captchaOff = { siteKey: '', token: '', setToken() {}, widget: { current: null }, reset() {}, missing: false };

describe('sign-up', () => {
  afterEach(() => cleanup());

  it('says creating an account agrees to the Terms and the Privacy Policy, linking both', () => {
    const fields = { displayName: '', email: '', password: '', setDisplayName() {}, setEmail() {}, setPassword() {} };
    const form = { error: '', submitting: false, setError() {} };
    render(<SignupForm fields={fields as never} form={form as never} captcha={captchaOff} onSubmit={() => {}} />);
    expect(screen.getByRole('link', { name: 'Terms of Service' }).getAttribute('href')).toBe('/terms');
    expect(screen.getByRole('link', { name: 'Privacy Policy' }).getAttribute('href')).toBe('/privacy');
  });
});
