/**
 * Unit tests for a topic page's navigation: an in-text jump goes to the heading when it is on the
 * page and to the full docs page when it is not, never nowhere.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { TopicNav } from '@/app/docs/_docs/topic-nav';
import { useDocsNav } from '@/app/docs/_docs/nav';

const push = vi.fn();
vi.mock('next/navigation', () => ({ useRouter: () => ({ push }) }));

afterEach(() => {
  cleanup();
  push.mockClear();
  history.replaceState(null, '', '/');
});

/** A part with one heading and jumps to it and to a heading on another page. */
function Part() {
  const { active, navigate } = useDocsNav();
  return (
    <>
      <h2 id="here">Here</h2>
      <button onClick={() => navigate('here')}>Stay</button>
      <button onClick={() => navigate('elsewhere')}>Leave</button>
      <output>{active}</output>
    </>
  );
}

describe('TopicNav', () => {
  it('sends a jump to a heading on another page to the full docs', async () => {
    render(
      <TopicNav>
        <Part />
      </TopicNav>,
    );
    await userEvent.click(screen.getByRole('button', { name: 'Leave' }));
    expect(push).toHaveBeenCalledWith('/docs#elsewhere');
  });

  it('jumps to a heading on this page by its anchor, without leaving', async () => {
    render(
      <TopicNav>
        <Part />
      </TopicNav>,
    );
    await userEvent.click(screen.getByRole('button', { name: 'Stay' }));
    expect(window.location.hash).toBe('#here');
    expect(push).not.toHaveBeenCalled();
  });

  it('marks no section active, since a topic page has no sidebar', () => {
    render(
      <TopicNav>
        <Part />
      </TopicNav>,
    );
    expect(screen.getByRole('status').textContent).toBe('');
  });
});
