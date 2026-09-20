/**
 * Unit tests for the toast provider: a toast shows, closes itself after its
 * lifetime or on its close button, and useToast needs the provider.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { act, cleanup, render, renderHook, screen, fireEvent } from '@testing-library/react';
import { ToastProvider, useToast } from '@/components/dashboard/toast';
import { TOAST_LIFETIME_MS } from '@/components/dashboard/toast/constants';
import { useToasts } from '@/components/dashboard/toast/use-toasts';

/** Raises one toast on mount of its button. */
function Raiser({ msg, type }: { msg: string; type?: 'success' | 'error' | 'info' }) {
  const toast = useToast();
  return <button onClick={() => toast(msg, type)}>raise</button>;
}

/** Renders a provider around a button that raises `msg`. */
const setup = (msg: string, type?: 'success' | 'error' | 'info') =>
  render(
    <ToastProvider>
      <Raiser msg={msg} type={type} />
    </ToastProvider>,
  );

describe('ToastProvider', () => {
  afterEach(() => {
    cleanup();
    vi.useRealTimers();
  });

  it('shows the message when a toast is raised', () => {
    setup('Saved', 'success');
    fireEvent.click(screen.getByText('raise'));
    expect(screen.getByText('Saved')).toBeTruthy();
  });

  it('colors an error toast red', () => {
    setup('Nope', 'error');
    fireEvent.click(screen.getByText('raise'));
    expect(screen.getByText('Nope').parentElement?.className).toContain('text-red-400');
  });

  it('refuses useToast outside a provider', () => {
    vi.spyOn(console, 'error').mockImplementation(() => {});
    expect(() => renderHook(() => useToast())).toThrow('useToast must be used within ToastProvider');
  });
});

describe('useToasts', () => {
  afterEach(() => vi.useRealTimers());

  it('removes a toast once its lifetime has passed', () => {
    vi.useFakeTimers();
    const { result } = renderHook(() => useToasts());
    act(() => result.current.toast('Gone soon'));
    expect(result.current.toasts.map((t) => t.msg)).toEqual(['Gone soon']);
    act(() => vi.advanceTimersByTime(TOAST_LIFETIME_MS));
    expect(result.current.toasts).toEqual([]);
  });

  it('removes a toast early on dismiss', () => {
    const { result } = renderHook(() => useToasts());
    act(() => result.current.toast('Close me', 'error'));
    act(() => result.current.dismiss(result.current.toasts[0].id));
    expect(result.current.toasts).toEqual([]);
  });

  it('shows a toast as a note unless told otherwise', () => {
    const { result } = renderHook(() => useToasts());
    act(() => result.current.toast('Hi'));
    expect(result.current.toasts[0].type).toBe('info');
  });
});
