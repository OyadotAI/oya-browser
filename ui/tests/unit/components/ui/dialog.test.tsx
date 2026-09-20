/**
 * Unit tests for the dialog: focus moves in and is trapped, Escape and the
 * backdrop close only the topmost dialog, scrolling locks while any is open,
 * and focus returns to the opener.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import Dialog, { Confirm } from '@/components/ui/dialog';

afterEach(cleanup);

/** jsdom lays nothing out, so every element reads as hidden; make them visible for the Tab trap. */
Object.defineProperty(HTMLElement.prototype, 'offsetParent', { get: () => document.body, configurable: true });

/** A dialog with two buttons inside. */
function TwoButtons({ open, onClose }: { open: boolean; onClose: () => void }) {
  return (
    <Dialog open={open} onClose={onClose} title="Settings" footer={<button>Save</button>}>
      <button>Inside</button>
    </Dialog>
  );
}

describe('Dialog', () => {
  it('names itself after its title and focuses the first control', () => {
    render(<TwoButtons open onClose={() => {}} />);
    expect(screen.getByRole('dialog', { name: 'Settings' })).toBeTruthy();
    expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Close' }));
  });

  it('closes on Escape, on the close button and on the backdrop, but not on a click inside', async () => {
    const onClose = vi.fn();
    render(<TwoButtons open onClose={onClose} />);
    await userEvent.keyboard('{Escape}');
    await userEvent.click(screen.getByRole('button', { name: 'Close' }));
    fireEvent.mouseDown(screen.getByRole('dialog'));
    fireEvent.mouseDown(screen.getByRole('dialog').parentElement!);
    expect(onClose).toHaveBeenCalledTimes(3);
  });

  it('keeps Tab inside, wrapping at both ends', async () => {
    render(<TwoButtons open onClose={() => {}} />);
    const save = screen.getByRole('button', { name: 'Save' });
    save.focus();
    await userEvent.tab();
    expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Close' }));
    await userEvent.tab({ shift: true });
    expect(document.activeElement).toBe(save);
  });

  it('only the topmost dialog answers Escape', async () => {
    const outer = vi.fn();
    const inner = vi.fn();
    render(
      <>
        <TwoButtons open onClose={outer} />
        <Dialog open onClose={inner} title="Inner">
          <p>x</p>
        </Dialog>
      </>,
    );
    await userEvent.keyboard('{Escape}');
    expect(inner).toHaveBeenCalledOnce();
    expect(outer).not.toHaveBeenCalled();
  });

  it('locks page scrolling while open and gives focus back to the opener on close', () => {
    document.body.style.overflow = 'auto';
    const opener = document.body.appendChild(document.createElement('button'));
    opener.focus();
    const { rerender } = render(<TwoButtons open onClose={() => {}} />);
    expect(document.body.style.overflow).toBe('hidden');
    rerender(<TwoButtons open={false} onClose={() => {}} />);
    expect(document.body.style.overflow).toBe('auto');
    expect(document.activeElement).toBe(opener);
    opener.remove();
  });
});

describe('Confirm', () => {
  it('confirms, cancels, and disables the go-ahead while busy', async () => {
    const onConfirm = vi.fn();
    const onClose = vi.fn();
    const { rerender } = render(
      <Confirm
        open
        onClose={onClose}
        onConfirm={onConfirm}
        title="Stop?"
        body="Gone for good"
        confirmLabel="Stop"
        danger
      />,
    );
    await userEvent.click(screen.getByRole('button', { name: 'Stop' }));
    await userEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(onConfirm).toHaveBeenCalledOnce();
    expect(onClose).toHaveBeenCalledOnce();
    rerender(<Confirm open onClose={onClose} onConfirm={onConfirm} title="Stop?" body="x" busy />);
    expect((screen.getByRole('button', { name: 'Working…' }) as HTMLButtonElement).disabled).toBe(true);
  });
});
