/**
 * Unit tests for the landing page, through what a visitor sees: the two next
 * steps, the proof, and the questions a buyer asks.
 */
import { describe, it, expect, afterEach, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';

/** jsdom has no IntersectionObserver; the scroll reveals only need one to exist. */
class NoIntersections {
  /** Nothing is ever observed intersecting. */
  observe() {}
  /** Nothing to stop observing. */
  unobserve() {}
  /** Nothing to release. */
  disconnect() {}
}
vi.stubGlobal('IntersectionObserver', NoIntersections);

vi.mock('@/components/ui/syntax-code', () => ({ default: ({ code }: { code: string }) => <code>{code}</code> }));

import Home from '@/app/page';
import { FAQ, START_STEPS, WALKTHROUGH, foundersCall } from '@/app/_home/content';
import { browserDownloads } from '@/lib/browser-downloads';

describe('landing page', () => {
  afterEach(() => cleanup());

  it('offers Start building and Talk to Founders, the founders one booking a call', () => {
    render(<Home />);
    const founders = screen.getAllByRole('link', { name: 'Talk to Founders' });
    expect(founders.length).toBeGreaterThanOrEqual(2);
    expect(founders.every((a) => a.getAttribute('href') === foundersCall)).toBe(true);
    expect(screen.getAllByRole('link', { name: /Start building/ })[0].getAttribute('href')).toBe('/dashboard');
  });

  it('puts a download button for every platform in the download section, with the header and hero pointing there', () => {
    render(<Home />);
    const downloads = screen.getByRole('navigation', { name: 'Browser downloads' });
    for (const { platform, href } of browserDownloads) {
      const link = screen.getByRole('link', { name: new RegExp(`Download for ${platform}`) });
      expect(downloads.contains(link) && link.getAttribute('href')).toBe(href);
    }
    expect(screen.getByRole('link', { name: 'Download' }).getAttribute('href')).toBe('/#download');
    expect(screen.getByRole('link', { name: /Download the app/ }).getAttribute('href')).toBe('#download');
  });

  it('numbers every step from download to a first task', () => {
    render(<Home />);
    const steps = screen.getByRole('list', { name: 'How to start' });
    expect(steps.querySelectorAll('li')).toHaveLength(START_STEPS.length);
    for (const { title } of START_STEPS) expect(steps.textContent).toContain(title);
  });

  it('says who it is for above the fold', () => {
    render(<Home />);
    expect(screen.getByText('For teams automating payer portals, EHRs and registries')).toBeTruthy();
    expect(screen.getByRole('heading', { level: 1 }).textContent).toMatch(/The portal has no API/);
  });

  it('answers every question in the FAQ', () => {
    render(<Home />);
    for (const { q } of FAQ) expect(screen.getByText(q)).toBeTruthy();
  });

  it('loads the walkthrough player only once play is pressed', () => {
    const { container } = render(<Home />);
    expect(container.querySelector('iframe')).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: /Watch the walkthrough/ }));
    expect(container.querySelector('iframe')?.getAttribute('src')).toBe(WALKTHROUGH.playing);
  });

  it('shows the replay running the generated code with no model calls', () => {
    render(<Home />);
    const shot = screen.getByRole('figure', { name: /playbook replaying/ });
    expect(shot.textContent).toContain('page.getByLabel("Member ID")');
    expect(screen.getByText('Model calls').nextElementSibling?.textContent).toBe('0');
  });
});
