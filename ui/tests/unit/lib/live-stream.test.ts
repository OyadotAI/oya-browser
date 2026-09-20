/**
 * Unit tests for the live-view frame stream: it buys a one-use ticket, opens
 * the stream with it, and reconnects after a failure until unsubscribed.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { subscribeFrames } from '@/lib/live-stream';
import { LIVE_RETRY_MS } from '@/lib/constants';
import { fakeFetch, fetchCall } from '../support';

/** A stand-in EventSource that records every instance. */
class FakeEventSource {
  /** Every stream opened, in order. */
  static opened: FakeEventSource[] = [];
  /** The URL it was opened with. */
  url: string;
  /** Whether close() was called. */
  closed = false;
  /** Frame handler. */
  onmessage: ((e: { data: string }) => void) | null = null;
  /** Error handler. */
  onerror: (() => void) | null = null;
  /** Records the stream. */
  constructor(url: string) {
    this.url = url;
    FakeEventSource.opened.push(this);
  }
  /** Marks it closed. */
  close() {
    this.closed = true;
  }
}

/** Lets pending promises settle. */
const settle = () => vi.advanceTimersByTimeAsync(0);

beforeEach(() => {
  vi.useFakeTimers();
  FakeEventSource.opened = [];
  vi.stubGlobal('EventSource', FakeEventSource);
});
afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe('subscribeFrames', () => {
  it('buys a ticket with the credential and opens the stream with the ticket, never the credential', async () => {
    const fn = fakeFetch({ body: { ticket: 't/1' } });
    const stop = subscribeFrames(
      'b 1',
      'key',
      () => {},
      () => {},
    );
    await settle();
    const [url, init] = fetchCall(fn);
    expect(url).toBe('/api/control/sessions/b%201/ticket');
    expect(init).toMatchObject({ method: 'POST', body: '{}' });
    expect(FakeEventSource.opened[0].url).toBe('/api/live/b%201?ticket=t%2F1');
    expect(FakeEventSource.opened[0].url).not.toContain('key');
    stop();
  });

  it('hands each frame to the subscriber', async () => {
    fakeFetch({ body: { ticket: 't' } });
    const onFrame = vi.fn();
    const stop = subscribeFrames('b', 'k', onFrame, () => {});
    await settle();
    FakeEventSource.opened[0].onmessage!({ data: 'frame-1' });
    expect(onFrame).toHaveBeenCalledWith('frame-1');
    stop();
  });

  it('reports a refused ticket and retries after the delay', async () => {
    const fn = fakeFetch({ status: 403, body: {} }, { body: { ticket: 't' } });
    const onError = vi.fn();
    const stop = subscribeFrames('b', 'k', () => {}, onError);
    await settle();
    expect(onError).toHaveBeenCalledOnce();
    await vi.advanceTimersByTimeAsync(LIVE_RETRY_MS - 1);
    expect(fn).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(1);
    expect(FakeEventSource.opened).toHaveLength(1);
    stop();
  });

  it('closes a dropped stream, reports it, and reconnects with a fresh ticket', async () => {
    const fn = fakeFetch({ body: { ticket: 't' } });
    const onError = vi.fn();
    const stop = subscribeFrames('b', 'k', () => {}, onError);
    await settle();
    FakeEventSource.opened[0].onerror!();
    expect(FakeEventSource.opened[0].closed).toBe(true);
    expect(onError).toHaveBeenCalledOnce();
    await vi.advanceTimersByTimeAsync(LIVE_RETRY_MS);
    expect(fn).toHaveBeenCalledTimes(2);
    expect(FakeEventSource.opened).toHaveLength(2);
    stop();
  });

  it('stops for good when unsubscribed: stream closed, no retry', async () => {
    const fn = fakeFetch({ body: { ticket: 't' } });
    const stop = subscribeFrames(
      'b',
      'k',
      () => {},
      () => {},
    );
    await settle();
    stop();
    expect(FakeEventSource.opened[0].closed).toBe(true);
    FakeEventSource.opened[0].onerror!();
    await vi.advanceTimersByTimeAsync(LIVE_RETRY_MS * 2);
    expect(fn).toHaveBeenCalledTimes(1);
  });

  it('opens no stream when unsubscribed while the ticket is in flight', async () => {
    fakeFetch({ body: { ticket: 't' } });
    const onError = vi.fn();
    const stop = subscribeFrames('b', 'k', () => {}, onError);
    stop();
    await settle();
    expect(FakeEventSource.opened).toHaveLength(0);
    expect(onError).not.toHaveBeenCalled();
  });
});
