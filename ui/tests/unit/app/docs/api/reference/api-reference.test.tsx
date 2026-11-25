/**
 * Unit tests for the interactive API reference (app/docs/api/reference): it waits
 * for Scalar's script and the spec, renders the spec with the request client off,
 * follows the site theme, and falls back to the raw spec when the script, the spec
 * or the clock runs out. Scalar and the CDN script are faked; nothing loads.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { act, cleanup, render, screen } from '@testing-library/react';

/**
 * What the faked next/script was last given, so a test can fire its load or error.
 * With `cached`, it calls onReady from its own effect, as next/script does on a return
 * visit: a child's effect runs before the page's, so before the page fetched anything.
 */
const script: { onReady?: () => void; onError?: () => void; props?: Record<string, unknown>; cached?: boolean } = {};
vi.mock('next/script', async () => {
  const { useEffect } = await import('react');
  /** Records its props; with `cached`, fires onReady from its own effect. */
  function FakeScript(props: Record<string, unknown>) {
    Object.assign(script, { onReady: props.onReady, onError: props.onError, props });
    useEffect(() => {
      if (script.cached) (props.onReady as () => void)();
    }, [props.onReady]);
    return null;
  }
  return { default: FakeScript };
});

import ApiReference from '@/app/docs/api/reference/api-reference';
import { LOAD_TIMEOUT_MS, SCALAR_INTEGRITY, SCALAR_SRC, specUrl } from '@/app/docs/api/reference/constants';

const SPEC = { openapi: '3.0.3', paths: {} };

/** A fake Scalar on window, recording what it was asked to render. */
function fakeScalar() {
  const instance = { updateConfiguration: vi.fn(), destroy: vi.fn() };
  const createApiReference = vi.fn((_el: HTMLElement, _config: object) => instance);
  window.Scalar = { createApiReference };
  return { createApiReference, instance };
}

/** Answers fetch with the spec, or with `status`. */
const serveSpec = (status = 200) =>
  vi.spyOn(globalThis, 'fetch').mockImplementation(async () => new Response(JSON.stringify(SPEC), { status }));

/** Renders the reference and lets the spec's fetch settle. */
async function setup() {
  render(<ApiReference nonce="n0nce" spec="/openapi.json" />);
  await act(async () => {});
}

describe('ApiReference', () => {
  beforeEach(() => {
    document.documentElement.dataset.theme = 'dark';
  });
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
    vi.useRealTimers();
    delete window.Scalar;
  });

  it('loads the pinned, hashed Scalar build with the page nonce', async () => {
    serveSpec();
    await setup();
    expect(script.props).toMatchObject({
      src: SCALAR_SRC,
      integrity: SCALAR_INTEGRITY,
      nonce: 'n0nce',
      crossOrigin: 'anonymous',
    });
    expect(screen.getByRole('status').textContent).toBe('Loading the API reference…');
  });

  it('renders the fetched spec with the request client and its key box turned off', async () => {
    serveSpec();
    const { createApiReference } = fakeScalar();
    await setup();
    await act(async () => script.onReady?.());
    const config = createApiReference.mock.calls[0][1];
    expect(config).toMatchObject({
      showDeveloperTools: 'never',
      agent: { disabled: true },
      telemetry: false,
      content: SPEC,
      hideClientButton: true,
      hideTestRequestButton: true,
      forceDarkModeState: 'dark',
    });
    expect(screen.queryByRole('status')).toBeNull();
  });

  it('renders on a return visit, where the script is ready before the page has fetched the spec', async () => {
    serveSpec();
    const { createApiReference } = fakeScalar();
    script.cached = true;
    render(<ApiReference spec="/openapi.json" />);
    await act(async () => {});
    script.cached = false;
    expect(createApiReference.mock.calls[0][1]).toMatchObject({ content: SPEC });
  });

  it('follows the site theme toggle', async () => {
    serveSpec();
    const { instance } = fakeScalar();
    await setup();
    await act(async () => script.onReady?.());
    document.documentElement.dataset.theme = 'light';
    act(() => void window.dispatchEvent(new Event('oya-theme')));
    expect(instance.updateConfiguration).toHaveBeenCalledWith(expect.objectContaining({ forceDarkModeState: 'light' }));
  });

  it('falls back to the raw spec when the spec cannot be fetched', async () => {
    serveSpec(500);
    await setup();
    expect(screen.getByRole('alert').textContent).toContain("The interactive reference didn't load.");
    expect(screen.getByRole('link', { name: 'Open /openapi.json' }).getAttribute('href')).toBe('/openapi.json');
  });

  it('falls back when the CDN script fails', async () => {
    serveSpec();
    await setup();
    act(() => script.onError?.());
    expect(screen.getByRole('alert')).toBeTruthy();
  });

  it('falls back when nothing has rendered in time', async () => {
    vi.useFakeTimers();
    serveSpec();
    await setup();
    await act(() => vi.advanceTimersByTimeAsync(LOAD_TIMEOUT_MS));
    expect(screen.getByRole('alert')).toBeTruthy();
  });
});

describe('specUrl', () => {
  it('reads the spec from the API origin when the API is on another one, else from this one', () => {
    expect(specUrl('https://api.example.test/api')).toBe('https://api.example.test/openapi.json');
    expect(specUrl('/api')).toBe('/openapi.json');
    expect(specUrl(undefined)).toBe('/openapi.json');
  });
});
