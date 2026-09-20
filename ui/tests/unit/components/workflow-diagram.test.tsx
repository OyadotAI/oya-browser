/**
 * Unit tests for the landing diagram: tabs switch stages by click and by
 * arrow, Home and End, and the repair stage shows its draft.
 */
import { describe, it, expect, afterEach } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import WorkflowDiagram from '@/components/workflow-diagram';

afterEach(cleanup);

describe('WorkflowDiagram', () => {
  it('starts on Record and moves with the arrows, wrapping, and Home and End', async () => {
    render(<WorkflowDiagram />);
    const record = screen.getByRole('tab', { name: 'Record' });
    expect(record.getAttribute('aria-selected')).toBe('true');
    record.focus();
    await userEvent.keyboard('{ArrowLeft}');
    expect(document.activeElement).toBe(screen.getByRole('tab', { name: 'Repair' }));
    await userEvent.keyboard('{ArrowRight}');
    expect(document.activeElement).toBe(record);
    await userEvent.keyboard('{End}');
    expect(screen.getByRole('tab', { name: 'Repair' }).getAttribute('aria-selected')).toBe('true');
    await userEvent.keyboard('{Home}');
    expect(record.getAttribute('aria-selected')).toBe('true');
  });

  it('shows the repair as a reviewable draft', async () => {
    render(<WorkflowDiagram />);
    await userEvent.click(screen.getByRole('tab', { name: 'Repair' }));
    expect(screen.getByRole('tabpanel').textContent).toContain('portal-request-review:draft');
    await userEvent.click(screen.getByRole('tab', { name: 'Replay' }));
    expect(screen.getByRole('tabpanel').textContent).not.toContain(':draft');
  });
});
