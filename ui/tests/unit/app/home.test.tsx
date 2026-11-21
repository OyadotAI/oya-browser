/**
 * Unit tests for the landing page, through what a visitor sees: the replay and its audit
 * strip, the steps, the workflow tabs, the trail, the questions, and the claims it may make.
 */
import { STACK_LINES } from '@/app/_home/stack-content';
import { describe, it, expect, afterEach, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

/** jsdom has no IntersectionObserver; the replay only needs one to exist. */
class NoIntersections {
  /** Nothing is ever observed intersecting. */
  observe(target?: Element) {
    void target;
  }
  /** Nothing to stop observing. */
  unobserve() {}
  /** Nothing to release. */
  disconnect() {}
}
vi.stubGlobal('IntersectionObserver', NoIntersections);

/** Tracks the header's layout listener so route cleanup is observable. */
const layoutListeners = new Set<() => void>();
vi.stubGlobal('matchMedia', (query: string) => ({
  matches: query.includes('min-width') || motion.reduced,
  addEventListener: (_event: string, listener: () => void) => {
    if (query.includes('min-width')) layoutListeners.add(listener);
  },
  removeEventListener: (_event: string, listener: () => void) => {
    if (query.includes('min-width')) layoutListeners.delete(listener);
  },
}));

/** An observer that reports everything on screen at once, so in-view motion starts. */
class InView extends NoIntersections {
  /** The callback to tell. */
  private readonly report: IntersectionObserverCallback;
  /** Keeps the callback. */
  constructor(report: IntersectionObserverCallback) {
    super();
    this.report = report;
  }
  /** Reports the element as intersecting straight away. */
  observe(target?: Element) {
    this.report([{ isIntersecting: true, target } as IntersectionObserverEntry], this as never);
  }
}

/** Whether the person asked for reduced motion; framer reads the OS setting once per process, so tests set it here. */
const motion = vi.hoisted(() => ({ reduced: false }));
vi.mock('framer-motion', async (original) => ({
  ...(await original<typeof import('framer-motion')>()),
  useReducedMotion: () => motion.reduced,
}));
vi.mock('next/headers', () => ({ headers: async () => new Headers() }));
vi.mock('next/font/google', () => ({
  IBM_Plex_Sans: () => ({ style: { fontFamily: 'Plex' } }),
  IBM_Plex_Mono: () => ({ style: { fontFamily: 'Plex Mono' } }),
}));

import Home from '@/app/page';
import { COMPLIANCE_CLAIMS, FAQ, PRODUCT_HUNT_BADGE, REPLAY, foundersCall } from '@/app/_home/content';
import { DEMO_LABELS, DEMO_TASKS } from '@/app/_home/demo-content';
import { PRODUCT_HUNT_BADGE_HEIGHT, PRODUCT_HUNT_BADGE_WIDTH, SHOWCASE_TICK_MS } from '@/app/_home/constants';
import { browserDownloads } from '@/lib/browser-downloads';

/** Renders the nonce-aware public page. */
const page = async () => render(await Home());
/** The currently selected synthetic workflow. */
const replay = () => screen.getByRole('figure', { name: /Example replay:/ });

describe('landing page', () => {
  afterEach(() => cleanup());

  it('introduces a first case and saved steps to healthtech teams and preserves the existing actions', async () => {
    await page();
    expect(screen.getByRole('heading', { level: 1 }).textContent).toBe(
      'Oya completes one payer portal case. You keep the steps as code.',
    );
    expect(screen.getByText('For healthtech and RCM technology teams')).toBeTruthy();
    expect(
      screen.getByText(
        'Start with a prior authorization, eligibility check or claim-status lookup, and Oya saves the successful steps as a playbook you can replay with new inputs.',
      ),
    ).toBeTruthy();
    const demos = screen.getAllByRole('link', { name: /Book a demo/ });
    expect(demos.length).toBeGreaterThanOrEqual(3);
    expect(demos.every((a) => a.getAttribute('href') === foundersCall)).toBe(true);
    expect(screen.getByRole('link', { name: 'Start building' }).getAttribute('href')).toBe('/dashboard');
  });

  it('places the linked compliance badge after the actions and keeps small print out of the hero', async () => {
    await page();
    const hero = screen.getByRole('heading', { level: 1 }).closest('section')!;
    const badge = within(hero).getByRole('link', { name: COMPLIANCE_CLAIMS.join(' · ') });
    const action = within(hero).getByRole('link', { name: 'Start building' });
    expect(within(hero).queryByText(/Use synthetic data while developing on desktop/)).toBeNull();
    expect(badge.getAttribute('href')).toBe('/trust');
    expect(badge.textContent).toBe(COMPLIANCE_CLAIMS.join(' · '));
    expect(action.compareDocumentPosition(badge) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it('renders the official dark Product Hunt launch badge lazily in the footer, not in the hero', async () => {
    await page();
    const hero = screen.getByRole('heading', { level: 1 }).closest('section')!;
    expect(within(hero).queryByRole('link', { name: PRODUCT_HUNT_BADGE.alt })).toBeNull();
    const link = within(screen.getByRole('contentinfo')).getByRole('link', { name: PRODUCT_HUNT_BADGE.alt });
    const img = within(link).getByRole('img', { name: PRODUCT_HUNT_BADGE.alt });
    expect(link.getAttribute('href')).toBe(PRODUCT_HUNT_BADGE.href);
    expect(link.getAttribute('target')).toBe('_blank');
    expect(link.getAttribute('rel')).toBe('noopener noreferrer');
    expect(link.getAttribute('data-track')).toBe('cta_clicked');
    expect(link.getAttribute('data-track-label')).toBe('footer_product_hunt');
    expect(img.getAttribute('src')).toBe(PRODUCT_HUNT_BADGE.image);
    expect(img.getAttribute('width')).toBe(String(PRODUCT_HUNT_BADGE_WIDTH));
    expect(img.getAttribute('height')).toBe(String(PRODUCT_HUNT_BADGE_HEIGHT));
    expect(img.getAttribute('loading')).toBe('lazy');
  });

  it('replaces the abstract artwork with exactly one playable hero recording and preserves headline and actions', async () => {
    const { container } = await page();
    expect(container.querySelector('img[src*="oya-routing-sculpture"]')).toBeNull();
    const videos = container.querySelectorAll('video');
    expect(videos).toHaveLength(1);
    expect(videos[0].closest('#hero')).toBeTruthy();
    expect(videos[0].closest('[aria-hidden="true"]')).toBeNull();
    expect(videos[0].controls).toBe(true);
    expect(screen.getAllByRole('heading', { level: 1 })).toHaveLength(1);
    expect(screen.getByRole('link', { name: 'Start building' }).getAttribute('href')).toBe('/dashboard');
  });

  it('starts on prior authorization and exposes three existing workflows through accessible tabs', async () => {
    await page();
    expect(screen.getByRole('heading', { name: 'Watch one case become a playbook.' })).toBeTruthy();
    const tabs = within(screen.getByRole('tablist', { name: 'Workflows' })).getAllByRole('tab');
    expect(tabs.map((tab) => tab.textContent)).toEqual(DEMO_LABELS);
    expect(tabs[0].getAttribute('aria-selected')).toBe('true');
    expect(replay().getAttribute('aria-label')).toContain(REPLAY.title);
    expect(screen.queryByRole('region', { name: 'Generated playbook' })).toBeNull();
    expect(screen.queryByRole('region', { name: 'Example audit trail' })).toBeNull();
  });

  it('changes the portal, result and requested code together when the task changes', async () => {
    await page();
    fireEvent.click(screen.getByRole('tab', { name: 'Eligibility' }));
    expect(replay().getAttribute('aria-label')).toContain(DEMO_TASKS[1].title);
    expect(within(screen.getByRole('status')).getByText(DEMO_TASKS[1].result)).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'View code' }));
    const code = screen.getByRole('region', { name: 'Generated playbook' });
    expect(code.textContent).toContain('getByLabel("Date of birth")');
    expect(code.textContent).not.toContain('getByLabel("Diagnosis")');
    fireEvent.click(screen.getByRole('tab', { name: 'Claim status' }));
    expect(replay().getAttribute('aria-label')).toContain(DEMO_TASKS[2].title);
    expect(within(screen.getByRole('status')).getByText(DEMO_TASKS[2].result)).toBeTruthy();
    expect(screen.getByRole('button', { name: 'View code' }).getAttribute('aria-expanded')).toBe('false');
  });

  it('keeps each example prompt, saved playbook, input names and result together without opening details', async () => {
    await page();
    const examples = [
      {
        tab: 'Prior authorization',
        prompt: /Submit a prior authorization for member/,
        file: 'prior-auth.ts',
        task: DEMO_TASKS[0],
      },
      {
        tab: 'Eligibility',
        prompt: /Check eligibility and benefits for member/,
        file: 'eligibility.ts',
        task: DEMO_TASKS[1],
      },
      {
        tab: 'Claim status',
        prompt: /Find the status and remittance details for claim/,
        file: 'claims.ts',
        task: DEMO_TASKS[2],
      },
    ];
    for (const example of examples) {
      fireEvent.click(screen.getByRole('tab', { name: example.tab }));
      const prompt = within(replay()).getByText(example.prompt);
      expect(prompt.textContent).toContain(example.task.fields[0].value);
      const artifact = screen.getByRole('group', { name: 'Saved playbook' });
      expect(artifact.textContent).toContain(example.file);
      for (const field of example.task.fields) expect(artifact.textContent).toContain(field.label);
      expect(screen.getByRole('status').textContent).toContain(example.task.result);
      expect(screen.queryByRole('region', { name: 'Generated playbook' })).toBeNull();
    }
  });

  it('shows each stack the one line that moves it onto Oya, from the content the docs share', async () => {
    const user = userEvent.setup();
    await page();
    const section = screen.getByRole('region', { name: 'Call the saved playbook from your own tools.' });
    const summary = within(section).getByText('View connection examples');
    const disclosure = summary.closest('details');
    expect(disclosure?.open).toBe(false);
    await user.click(summary);
    expect(disclosure?.open).toBe(true);
    for (const { line, who } of STACK_LINES) {
      expect(section.textContent).toContain(line);
      expect(section.textContent).toContain(who);
    }
    await user.click(summary);
    expect(disclosure?.open).toBe(false);
  });

  it('connects the first workflow and its results to operating, controlling and deploying more workflows', async () => {
    const { container } = await page();
    const journey = [
      'hero',
      'recorded-demo',
      'workflows',
      'results',
      'stack',
      'control-plane',
      'capabilities',
      'deployment',
      'pricing',
      'faq',
      'contact',
    ];
    const order = Array.from(container.querySelectorAll('[id]'))
      .map((element) => element.id)
      .filter((id) => journey.includes(id));
    expect(order).toEqual(journey);
    expect(screen.getAllByRole('heading', { level: 2 }).map((heading) => heading.textContent)).toEqual([
      'Watch one case become a playbook.',
      'A prior authorization in under 2 minutes, not 22 to 26.',
      'When the portal changes, Oya autoheals the code.',
      'Call the saved playbook from your own tools.',
      'Run it under your controls, in our cloud or yours.',
      'Start with the free desktop app.',
      'Questions before you start.',
      'Scope your first workflow with the founders.',
    ]);
    const architecture = screen.getByRole('region', { name: 'Run it under your controls, in our cloud or yours.' });
    expect(
      within(architecture)
        .getAllByRole('heading', { level: 3 })
        .map((heading) => heading.textContent),
    ).toEqual(['Agents & apps', 'Oya control plane', 'Browser fleet', 'Healthcare portals']);
    expect(screen.getByText('Interactive simulation · Synthetic data')).toBeTruthy();
  });

  it('nests run controls and deployment under the one operations heading', async () => {
    await page();
    for (const name of ['Decide how each run should work.', 'Choose where the work runs.'])
      expect(screen.getByRole('heading', { name, level: 3 })).toBeTruthy();
    for (const name of ['Oya Cloud', 'Execution choice'])
      expect(screen.getByRole('heading', { name, level: 4 })).toBeTruthy();
  });

  it('shows only the selected task’s synthetic audit actions and references', async () => {
    await page();
    fireEvent.click(screen.getByRole('tab', { name: 'Eligibility' }));
    fireEvent.click(screen.getByRole('button', { name: 'View audit trail' }));
    const audit = screen.getByRole('region', { name: 'Example audit trail' });
    for (const row of DEMO_TASKS[1].ledger) expect(audit.textContent).toContain(row.hash);
    expect(audit.textContent).toContain('Filled Date of birth');
    expect(audit.textContent).not.toContain('Filled diagnosis M54.50');
    expect(audit.textContent).not.toContain(REPLAY.ledger[0].hash);
    const label = screen.getByText('Interactive simulation · Synthetic data');
    expect(label.compareDocumentPosition(replay()) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it('wraps task arrow navigation and keeps selection, focus and the single tab stop together', async () => {
    await page();
    const tabs = within(screen.getByRole('tablist', { name: 'Workflows' })).getAllByRole('tab');
    fireEvent.keyDown(tabs[0], { key: 'ArrowLeft' });
    expect(document.activeElement).toBe(tabs.at(-1));
    expect(tabs.at(-1)?.tabIndex).toBe(0);
    expect(tabs[0].tabIndex).toBe(-1);
    fireEvent.keyDown(tabs.at(-1)!, { key: 'ArrowRight' });
    expect(document.activeElement).toBe(tabs[0]);
    fireEvent.keyDown(tabs[0], { key: 'End' });
    expect(document.activeElement).toBe(tabs.at(-1));
    fireEvent.keyDown(tabs.at(-1)!, { key: 'Home' });
    expect(document.activeElement).toBe(tabs[0]);
  });

  it('dismisses evidence with Escape and Close details, restoring its trigger focus', async () => {
    const user = userEvent.setup();
    await page();
    const code = screen.getByRole('button', { name: 'View code' });
    await user.click(code);
    await user.keyboard('{Escape}');
    expect(code.getAttribute('aria-expanded')).toBe('false');
    expect(document.activeElement).toBe(code);
    const audit = screen.getByRole('button', { name: 'View audit trail' });
    await user.click(audit);
    await user.click(screen.getByRole('button', { name: 'Close details' }));
    expect(audit.getAttribute('aria-expanded')).toBe('false');
    expect(document.activeElement).toBe(audit);
  });

  it('shows completed examples with reduced motion and no replay pause control', async () => {
    motion.reduced = true;
    try {
      await page();
      expect(screen.queryByRole('button', { name: /Pause replay|Play replay/ })).toBeNull();
      for (const name of [
        'Describe the task you already know.',
        'Watch the agent work through it.',
        'Keep the steps, not just the result.',
        'Use those steps for the next case.',
      ]) {
        expect(within(replay()).getByRole('heading', { level: 3, name })).toBeTruthy();
      }
      expect(screen.getByRole('group', { name: 'Saved playbook' }).textContent).toContain(REPLAY.file);
      expect(within(screen.getByRole('status')).getByText(REPLAY.result)).toBeTruthy();
      fireEvent.click(screen.getByRole('tab', { name: 'Claim status' }));
      expect(within(screen.getByRole('status')).getByText(DEMO_TASKS[2].result)).toBeTruthy();
    } finally {
      motion.reduced = false;
    }
  });

  it('pauses a replay, resets it on task selection and releases its timer on unmount', async () => {
    vi.useFakeTimers();
    vi.stubGlobal('IntersectionObserver', InView);
    try {
      const view = await page();
      act(() => vi.advanceTimersByTime(SHOWCASE_TICK_MS * 2));
      fireEvent.click(screen.getByRole('button', { name: 'Pause replay' }));
      expect(screen.getByRole('button', { name: 'Play replay' })).toBeTruthy();
      expect(screen.getByText('Agent paused')).toBeTruthy();
      expect(within(replay()).getByText('Paused, no model')).toBeTruthy();
      expect(within(replay()).getByText('Example workflow paused.')).toBeTruthy();
      fireEvent.click(screen.getByRole('tab', { name: 'Eligibility' }));
      expect(screen.getByRole('button', { name: 'Pause replay' })).toBeTruthy();
      expect(within(replay()).queryByText(REPLAY.fields[1].value)).toBeNull();
      view.unmount();
      expect(vi.getTimerCount()).toBe(0);
    } finally {
      vi.stubGlobal('IntersectionObserver', NoIntersections);
      vi.useRealTimers();
    }
  });

  it('makes every original FAQ answer accessible on demand, including pricing and PHI restrictions', async () => {
    const user = userEvent.setup();
    await page();
    for (const { q, a } of FAQ) {
      const button = screen.getByRole('button', { name: q });
      expect(button.getAttribute('aria-expanded')).toBe('false');
      button.focus();
      await user.keyboard('{Enter}');
      expect(screen.getByRole('region', { name: q }).textContent).toBe(a);
      expect(button.getAttribute('aria-expanded')).toBe('true');
    }
  });

  it('compares the same task with clear units and removes the overlapping 10-times-faster metric', async () => {
    await page();
    const comparison = screen.getByRole('group', { name: 'Time per prior authorization.' });
    expect(comparison.textContent).toContain('By hand22–26 minutes');
    expect(comparison.textContent).toContain('With OyaUnder 2 minutes');
    expect(comparison.textContent).not.toContain('10 to 30');
    expect(comparison.textContent).not.toContain('10 times faster');
    const throughput = screen.getByRole('group', { name: 'Reported throughput' });
    expect(throughput.textContent).toContain('Customers report completing 10 to 30 times as many tasks per hour');
    expect(throughput.textContent).not.toContain('prior authorization');
    expect(throughput.textContent).not.toContain('Under 2 minutes');
  });

  it('claims SOC 2 as the Type I report or the Type II audit in progress, never certified, and preserves the HIPAA wording', async () => {
    const { container } = await page();
    const text = container.textContent ?? '';
    expect(text).not.toMatch(/certified|SOC 2 (compliant|report|attested)/i);
    for (const [at] of text.matchAll(/SOC 2[^.·]*/g))
      expect(at).toMatch(/^SOC 2 (Type I(?!I)|Type II audit (is )?in progress)/);
    expect(text).toContain('HIPAA compliant');
  });

  it('keeps existing navigation anchors and platform downloads reachable', async () => {
    await page();
    for (const id of ['top', 'playbooks', 'steps', 'audit', 'workflows', 'pricing', 'faq', 'contact', 'download'])
      expect(document.getElementById(id)).toBeTruthy();
    const downloads = within(screen.getByRole('navigation', { name: 'Browser downloads' }));
    for (const { platform, href } of browserDownloads)
      expect(downloads.getByRole('link', { name: new RegExp(platform) }).getAttribute('href')).toBe(href);
    // Version-pinned file names went stale and 404ed; only the server's version-free routes stay current.
    expect(browserDownloads.map(({ href }) => href)).toEqual(['/download/mac', '/download/windows']);
  });

  it('keeps the main navigation focused and secondary destinations reachable in the footer and Product menu', async () => {
    await page();
    const main = within(screen.getByRole('navigation', { name: 'Main' }));
    expect(main.getAllByRole('link').map((link) => link.textContent)).toEqual([
      'Workflows',
      'Pricing',
      'Docs',
      'Security',
    ]);
    expect(main.getByRole('link', { name: 'Pricing' }).getAttribute('href')).toBe('/pricing');
    fireEvent.click(screen.getByRole('button', { name: 'Product' }));
    expect(main.getByRole('link', { name: /^Control plane/ }).getAttribute('href')).toBe('/#control-plane');
    expect(main.getByRole('button', { name: 'Product' }).getAttribute('aria-expanded')).toBe('true');
    const footer = within(screen.getByRole('contentinfo'));
    for (const [name, href] of [
      ['Compare', '/vs'],
      ['Research', '/research'],
      ['Pricing', '/pricing'],
      ['Questions', '/#faq'],
    ])
      expect(footer.getByRole('link', { name }).getAttribute('href')).toBe(href);
  });

  it('opens Product destinations and restores trigger focus on Escape', async () => {
    await page();
    const toggle = screen.getByRole('button', { name: 'Product' });
    toggle.focus();
    fireEvent.click(toggle);
    const security = screen.getByRole('link', { name: 'Security HIPAA, BAAs, the audit' });
    expect(security.getAttribute('href')).toBe('/trust');
    security.focus();
    fireEvent.keyDown(security, { key: 'Escape' });
    expect(toggle.getAttribute('aria-expanded')).toBe('false');
    expect(document.activeElement).toBe(toggle);
  });

  it('keeps the menu open when a tap blurs focus without moving it, as Safari does', async () => {
    await page();
    const menu = screen.getByRole('button', { name: 'Menu' });
    fireEvent.click(menu);
    const product = within(screen.getByRole('navigation', { name: 'Main' })).getByRole('button', { name: 'Product' });
    fireEvent.blur(product, { relatedTarget: null });
    expect(menu.getAttribute('aria-expanded')).toBe('true');
    fireEvent.click(screen.getByRole('button', { name: 'Product' }));
    expect(screen.getByRole('button', { name: 'Product' }).getAttribute('aria-expanded')).toBe('true');
  });

  it('closes navigation when focus moves outside the header', async () => {
    await page();
    const menu = screen.getByRole('button', { name: 'Menu' });
    fireEvent.click(menu);
    const product = within(screen.getByRole('navigation', { name: 'Main' })).getByRole('button', { name: 'Product' });
    fireEvent.blur(product, { relatedTarget: document.body });
    expect(menu.getAttribute('aria-expanded')).toBe('false');
  });

  it('moves forward through revealed menu destinations and restores Menu on Escape', async () => {
    const user = userEvent.setup();
    await page();
    const menu = screen.getByRole('button', { name: 'Menu' });
    menu.focus();
    await user.keyboard('{Enter}');
    const navigation = within(screen.getByRole('navigation', { name: 'Main' }));
    expect(document.activeElement).toBe(navigation.getByRole('button', { name: 'Product' }));
    await user.tab();
    expect(document.activeElement).toBe(navigation.getByRole('link', { name: 'Workflows' }));
    await user.keyboard('{Escape}');
    expect(menu.getAttribute('aria-expanded')).toBe('false');
    expect(document.activeElement).toBe(menu);
  });

  it('closes navigation on a destination or layout change and releases listeners when leaving', async () => {
    const view = await page();
    const menu = screen.getByRole('button', { name: 'Menu' });
    fireEvent.click(menu);
    fireEvent.click(within(screen.getByRole('navigation', { name: 'Main' })).getByRole('link', { name: 'Workflows' }));
    expect(menu.getAttribute('aria-expanded')).toBe('false');
    fireEvent.click(menu);
    act(() => layoutListeners.forEach((listener) => listener()));
    expect(menu.getAttribute('aria-expanded')).toBe('false');
    expect(document.body.classList.contains('nav-locked')).toBe(false);
    expect(layoutListeners.size).toBe(1);
    view.unmount();
    expect(layoutListeners.size).toBe(0);
  });

  it('reveals enterprise governance and observability detail without losing the other group summaries', async () => {
    await page();
    const tabs = within(screen.getByRole('tablist', { name: 'Control plane capabilities' }));
    const govern = tabs.getByRole('tab', { name: /^Set access/ });
    fireEvent.click(govern);
    expect(screen.getByRole('tabpanel', { name: /^Set access/ }).textContent).toContain(
      'Administrator, operator and viewer',
    );
    expect(screen.getByRole('tabpanel', { name: /^Set access/ }).textContent).toContain('managed runtime');
    fireEvent.keyDown(govern, { key: 'ArrowDown' });
    expect(screen.getByRole('tabpanel', { name: /^Review runs/ }).textContent).toContain('Signed lifecycle webhooks');
    expect(document.activeElement).toBe(tabs.getByRole('tab', { name: /^Review runs/ }));
    expect(tabs.getAllByRole('tab')).toHaveLength(3);
  });

  it('keeps the agent paused during human control and after release until explicit resume', async () => {
    vi.useFakeTimers();
    vi.stubGlobal('IntersectionObserver', InView);
    try {
      await page();
      act(() => vi.advanceTimersByTime(SHOWCASE_TICK_MS * 2));
      fireEvent.click(screen.getByRole('button', { name: 'Try human takeover' }));
      expect(screen.getByText('Human control · Agent paused')).toBeTruthy();
      const human = replay().textContent;
      act(() => vi.advanceTimersByTime(SHOWCASE_TICK_MS * 3));
      expect(replay().textContent).toBe(human);
      fireEvent.click(screen.getByRole('button', { name: 'Release control' }));
      expect(screen.getByText('Agent paused')).toBeTruthy();
      expect(screen.queryByRole('button', { name: 'Play replay' })).toBeNull();
      const released = replay().textContent;
      act(() => vi.advanceTimersByTime(SHOWCASE_TICK_MS * 3));
      expect(replay().textContent).toBe(released);
      fireEvent.click(screen.getByRole('button', { name: 'Resume agent' }));
      expect(screen.getByText('Agent running')).toBeTruthy();
      const resumed = replay().textContent;
      act(() => vi.advanceTimersByTime(SHOWCASE_TICK_MS * 3));
      expect(replay().textContent).not.toBe(resumed);
    } finally {
      vi.stubGlobal('IntersectionObserver', NoIntersections);
      vi.useRealTimers();
    }
  });

  it('bounds repeated ownership events and resets the example on restart and task changes', async () => {
    await page();
    for (let index = 0; index < 3; index++) {
      fireEvent.click(screen.getByRole('button', { name: 'Try human takeover' }));
      fireEvent.click(screen.getByRole('button', { name: 'Release control' }));
      fireEvent.click(screen.getByRole('button', { name: 'Resume agent' }));
    }
    expect(
      within(screen.getByRole('list', { name: 'Example ownership events' })).getAllByRole('listitem'),
    ).toHaveLength(4);
    fireEvent.click(screen.getByRole('button', { name: 'Restart example' }));
    expect(screen.queryByRole('list', { name: 'Example ownership events' })).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'Try human takeover' }));
    fireEvent.click(screen.getByRole('tab', { name: 'Eligibility' }));
    expect(screen.getByText('Agent running')).toBeTruthy();
    expect(screen.queryByRole('list', { name: 'Example ownership events' })).toBeNull();
  });

  it('states material runtime boundaries alongside deployment choices', async () => {
    await page();
    const limits = screen.getByText('Runtime limits').closest('details');
    expect(limits?.textContent).toContain('Strict host-policy enforcement requires the managed runtime');
    expect(limits?.textContent).toContain('not provider invoice guarantees');
    expect(limits?.textContent).toContain('Device-bound');
    expect(limits?.textContent).toContain('deduplicate');
  });

  it('keeps the enterprise platform story visible without loading document-wide landing assets', async () => {
    const { container } = await page();
    expect(container.querySelector('link[rel="stylesheet"], script')).toBeNull();
    expect(container.querySelector('style')?.textContent).toContain('--font-dm-sans');
    expect(container.querySelector('style')?.textContent).not.toContain(':root');
    expect(screen.getByRole('figure', { name: /Oya architecture/ })).toBeTruthy();
    const tabs = within(screen.getByRole('tablist', { name: 'Control plane capabilities' }));
    for (const name of ['Run work', 'Set access', 'Review runs'])
      expect(tabs.getByRole('tab', { name: new RegExp(name) })).toBeTruthy();
    expect(screen.getByRole('heading', { name: 'Choose where the work runs.' })).toBeTruthy();
  });
});
