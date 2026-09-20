/**
 * Unit tests for the live view: pointer and keyboard input become ordered
 * browser commands, the keyboard is captured and released, and the wheel is
 * gathered into scroll commands.
 */
import { describe, it, expect, vi, afterEach, beforeAll, beforeEach } from 'vitest';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import LiveView from '@/components/dashboard/live-view';
import type { Send } from '@/components/dashboard/live/types';
import { SCROLL_FLUSH_MS, TYPE_FLUSH_MS } from '@/components/dashboard/live/constants';

const CONTROL = 'Live view — click to control, Esc to release the keyboard';

beforeAll(() => {
  // A 1000×1000 page drawn in a 500×500 box at the origin: page = 2 × screen.
  Object.defineProperty(HTMLImageElement.prototype, 'naturalWidth', { get: () => 1000, configurable: true });
  Object.defineProperty(HTMLImageElement.prototype, 'naturalHeight', { get: () => 1000, configurable: true });
  HTMLImageElement.prototype.getBoundingClientRect = () => ({ left: 0, top: 0, width: 500, height: 500 }) as DOMRect;
});

/** Renders the view with spies; resolves pending promises with `flush`. */
function setup(props: { interactive?: boolean; send?: ReturnType<typeof vi.fn<Send>> } = {}) {
  const send = props.send ?? vi.fn<Send>(async () => ({}));
  const onInput = vi.fn();
  const view = render(
    <LiveView frameSrc="data:," fps={2} frameAgeMs={0} send={send} onInput={onInput} interactive={props.interactive} />,
  );
  const box = () => screen.getByLabelText(/^Live view/);
  return { send, onInput, box, view };
}

/** Lets queued commands run. */
const settle = () => act(async () => {});

/** Presses and releases the primary button at a screen point. */
function click(box: HTMLElement, x: number, y: number, toX = x, toY = y) {
  fireEvent.mouseDown(box, { button: 0, clientX: x, clientY: y });
  fireEvent.mouseUp(box, { button: 0, clientX: toX, clientY: toY });
}

describe('LiveView', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
    cleanup();
  });

  it('sends a click in page pixels and captures the keyboard', async () => {
    const { send, onInput, box } = setup();
    click(box(), 50, 100);
    await settle();
    expect(send).toHaveBeenCalledWith('click_coordinates', { x: 100, y: 200 });
    expect(onInput).toHaveBeenCalledWith('click 100,200');
    expect(box().hasAttribute('data-captures-keys')).toBe(true);
  });

  it('sends a drag when the press moved past the threshold', async () => {
    const { send, box } = setup();
    click(box(), 10, 10, 60, 10);
    await settle();
    expect(send).toHaveBeenCalledWith('drag', { from_x: 20, from_y: 20, to_x: 120, to_y: 20 });
  });

  it('batches typed characters into one command once typing pauses', async () => {
    const { send, box } = setup();
    click(box(), 1, 1);
    for (const key of 'hi') fireEvent.keyDown(box(), { key });
    await act(async () => vi.advanceTimersByTime(TYPE_FLUSH_MS));
    expect(send).toHaveBeenLastCalledWith('keyboard_type', { text: 'hi' });
    expect(send.mock.calls.filter(([a]) => a === 'keyboard_type')).toHaveLength(1);
  });

  it('sends typed text before a named key, in order', async () => {
    const { send, box } = setup();
    click(box(), 1, 1);
    fireEvent.keyDown(box(), { key: 'a' });
    fireEvent.keyDown(box(), { key: 'Enter' });
    await settle();
    expect(send.mock.calls.map(([a]) => a)).toEqual(['click_coordinates', 'keyboard_type', 'press_key']);
    expect(send).toHaveBeenLastCalledWith('press_key', { key: 'Enter' });
  });

  it('ignores keys until the view is clicked, and lets modifier shortcuts through', async () => {
    const { send, box } = setup();
    fireEvent.keyDown(box(), { key: 'x' });
    click(box(), 1, 1);
    fireEvent.keyDown(box(), { key: 'c', metaKey: true });
    await act(async () => vi.advanceTimersByTime(TYPE_FLUSH_MS));
    expect(send.mock.calls.map(([a]) => a)).toEqual(['click_coordinates']);
  });

  it('releases the keyboard on Escape without sending it', async () => {
    const { send, box } = setup();
    click(box(), 1, 1);
    fireEvent.keyDown(box(), { key: 'Escape' });
    await settle();
    expect(box().hasAttribute('data-captures-keys')).toBe(false);
    expect(send).not.toHaveBeenCalledWith('press_key', expect.anything());
  });

  it('keeps sending after a command fails', async () => {
    const send = vi.fn<Send>().mockRejectedValueOnce(new Error('gone')).mockResolvedValue({});
    const { box } = setup({ send });
    click(box(), 1, 1);
    fireEvent.keyDown(box(), { key: 'Tab' });
    await settle();
    expect(send).toHaveBeenLastCalledWith('press_key', { key: 'Tab' });
  });

  it('is watch-only while the agent has control, and releases the keyboard when control is lost', async () => {
    const { send, box, view } = setup();
    click(box(), 1, 1);
    view.rerender(<LiveView frameSrc="data:," fps={2} frameAgeMs={0} send={send} interactive={false} />);
    expect(
      screen.getByLabelText('Live view — watch only while the agent has control').hasAttribute('data-captures-keys'),
    ).toBe(false);
    click(box(), 5, 5);
    await settle();
    expect(send).toHaveBeenCalledTimes(1);
  });

  it('gathers small wheel deltas into one whole-pixel scroll', async () => {
    const { send, box } = setup();
    for (let i = 0; i < 3; i++) fireEvent.wheel(box(), { deltaY: 0.5, clientX: 10, clientY: 10 });
    await act(async () => vi.advanceTimersByTime(SCROLL_FLUSH_MS));
    expect(send).toHaveBeenCalledWith('scroll', {
      direction: 'down',
      amount: 1,
      x: 20,
      y: 20,
      smooth: false,
      analyze: false,
    });
  });

  it('counts line-mode wheel deltas as 16px lines', async () => {
    const { send, box } = setup();
    fireEvent.wheel(box(), { deltaY: -2, deltaMode: 1, clientX: 10, clientY: 10 });
    await act(async () => vi.advanceTimersByTime(SCROLL_FLUSH_MS));
    expect(send).toHaveBeenCalledWith('scroll', expect.objectContaining({ direction: 'up', amount: 32 }));
  });

  it('shows the stream status and warns about an old frame', () => {
    render(<LiveView frameSrc="data:," fps={0} frameAgeMs={9000} send={vi.fn<Send>()} />);
    expect(screen.getByText('idle')).toBeTruthy();
    expect(screen.getByText('last frame 9s ago')).toBeTruthy();
    expect(screen.getByLabelText(CONTROL)).toBeTruthy();
  });

  it('toggles mouse streaming and fit from the toolbar', () => {
    setup();
    fireEvent.click(screen.getByTitle('Stream mouse movement (uses bandwidth)'));
    expect(screen.getByTitle('Stream mouse movement (uses bandwidth)').getAttribute('aria-pressed')).toBe('true');
    fireEvent.click(screen.getByTitle('Actual size'));
    expect(screen.getByTitle('Fit to panel')).toBeTruthy();
  });
});
