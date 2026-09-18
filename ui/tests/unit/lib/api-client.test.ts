/**
 * Unit tests for the project API client: bodies are parsed, failures carry
 * the server's reason, and a refused credential is announced.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { api, ApiError, errorMessage, ago, shortId } from '@/lib/api-client';
import { fakeFetch, fetchCall } from '../support';

afterEach(() => vi.unstubAllGlobals());

describe('api', () => {
  it('sends the credential as a bearer token and the body as JSON', async () => {
    const fn = fakeFetch({ body: { ok: true } });
    await api('/browsers/stop', { key: 'k1', method: 'POST', body: { ids: ['a'] } });
    const [url, init] = fetchCall(fn);
    expect(url).toBe('/api/browsers/stop');
    expect(init.method).toBe('POST');
    expect((init.headers as Record<string, string>).Authorization).toBe('Bearer k1');
    expect(init.body).toBe('{"ids":["a"]}');
  });

  it('defaults to GET with no body', async () => {
    const fn = fakeFetch({ body: [] });
    await api('/browsers', { key: 'k' });
    expect(fetchCall(fn)[1]).toMatchObject({ method: 'GET', body: undefined });
  });

  it('returns parsed JSON, raw text when not JSON, and null when empty', async () => {
    fakeFetch({ body: { a: 1 } }, { body: 'plain text' }, { body: '' });
    expect(await api('/x', { key: 'k' })).toEqual({ a: 1 });
    expect(await api('/x', { key: 'k' })).toBe('plain text');
    expect(await api('/x', { key: 'k' })).toBeNull();
  });

  it("fails with the server's own reason, status and body", async () => {
    fakeFetch({ status: 409, body: { error: 'Persona is in use' } });
    const err = await api('/personas/p', { key: 'k', method: 'DELETE' }).catch((e) => e);
    expect(err).toBeInstanceOf(ApiError);
    expect(err).toMatchObject({ message: 'Persona is in use', status: 409, body: { error: 'Persona is in use' } });
  });

  it('names the request and status when the server gives no reason', async () => {
    fakeFetch({ status: 500, body: '' });
    await expect(api('/fleet', { key: 'k' })).rejects.toThrow('GET /fleet failed (500)');
  });

  it.each([401, 403, 410])('announces a refused credential on %i', async (status) => {
    fakeFetch({ status, body: {} });
    const heard = vi.fn();
    window.addEventListener('oya:credential-gone', heard);
    await api('/x', { key: 'k' }).catch(() => {});
    window.removeEventListener('oya:credential-gone', heard);
    expect(heard).toHaveBeenCalledOnce();
  });

  it('does not announce anything for other failures', async () => {
    fakeFetch({ status: 404, body: {} });
    const heard = vi.fn();
    window.addEventListener('oya:credential-gone', heard);
    await api('/x', { key: 'k' }).catch(() => {});
    window.removeEventListener('oya:credential-gone', heard);
    expect(heard).not.toHaveBeenCalled();
  });
});

describe('errorMessage', () => {
  it("uses an Error's message and a fallback for anything else", () => {
    expect(errorMessage(new Error('boom'))).toBe('boom');
    expect(errorMessage('nope')).toBe('Something went wrong');
    expect(errorMessage(null, 'custom')).toBe('custom');
  });
});

describe('re-exported formatters', () => {
  it('are still reachable from api-client', () => {
    expect(ago(null)).toBe('—');
    expect(shortId('abc')).toBe('abc');
  });
});
