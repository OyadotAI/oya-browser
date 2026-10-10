/**
 * Unit tests for the trust center, through what a visitor sees: every control the
 * evidence pack reports, with its gaps shown as gaps, and every subprocessor.
 */
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { describe, it, expect, afterEach } from 'vitest';
import { cleanup, render, screen, within } from '@testing-library/react';
import TrustCenter from '@/app/trust/page';
import { SUBPROCESSORS } from '@/app/trust/content';

/** The evidence pack the page is built from. */
const evidence = JSON.parse(readFileSync(path.join(process.cwd(), '..', 'compliance', 'evidence.json'), 'utf8'));

describe('trust center', () => {
  afterEach(() => cleanup());

  it('shows every control in the evidence pack, a failed or gap verdict as a gap', () => {
    render(<TrustCenter />);
    for (const control of evidence.controls) {
      const row = screen.getByText(`§${control.id}`).closest('tr') as HTMLElement;
      expect(within(row).getByText(control.verdict === 'PASS' ? 'Met' : 'Gap')).toBeTruthy();
    }
  });

  it('lists every subprocessor', () => {
    render(<TrustCenter />);
    for (const { name } of SUBPROCESSORS) expect(screen.getByText(name)).toBeTruthy();
  });

  it('is linked from the site footer', () => {
    render(<TrustCenter />);
    const footer = screen.getByRole('navigation', { name: 'Footer navigation' });
    expect(within(footer).getByRole('link', { name: 'Trust' }).getAttribute('href')).toBe('/trust');
  });
});
