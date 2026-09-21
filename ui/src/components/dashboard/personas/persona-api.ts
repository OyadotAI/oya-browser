/**
 * The profile and proxy endpoints the profile screens call, one function per
 * request, so the hooks read as steps rather than URLs.
 */
import { api } from '@/lib/api-client';
import type { Persona } from '../types';
import { mfaBody, mfaReady, type MfaDraft } from './mfa';
import {
  capValue,
  type CheckAnswer,
  type CredentialDraft,
  type DrawerFields,
  type Options,
  type PersonaDraft,
  type PreviewAnswer,
  type ProxyDraft,
  type ProxyList,
  type ProxyRow,
  type ProxyTable,
} from './model';

/** The device choices a new profile may make. */
export const fetchOptions = (key: string) => api<Options>('/personas/options', { key });

/** The device the server would generate for these preferences. */
export const fetchPreview = (key: string, prefs: Record<string, string>, signal: AbortSignal) =>
  api<PreviewAnswer>('/personas/preview', { key, method: 'POST', body: { prefs }, signal }).then((r) => r.fingerprint);

/** Creates the profile, then stores its second factor when one was filled in. */
export async function createPersona(key: string, draft: PersonaDraft, prefs: Record<string, string>) {
  const body: Record<string, unknown> = { name: draft.name || undefined, prefs, maxConcurrent: capValue(draft.cap) };
  if (draft.geo) body.proxy = { geo: draft.geo };
  const p = await api<Persona>('/personas', { key, method: 'POST', body });
  if (mfaReady(draft.mfa)) await storeMfa(key, p.id, draft.mfa);
  return p;
}

/** Saves the drawer's name, cap and geo hint. */
export const updatePersona = (key: string, p: Persona, f: DrawerFields) =>
  api(`/personas/${p.id}`, {
    key,
    method: 'PUT',
    body: { name: f.name, maxConcurrent: capValue(f.cap), proxy: f.geo ? { ...(p.proxy || {}), geo: f.geo } : null },
  });

/** Pins the persona to a proxy, or unpins it with a blank id. */
export const pinProxy = (key: string, id: string, proxyId: string) =>
  api(`/personas/${id}/proxy`, { key, method: 'PUT', body: { proxyId: proxyId || null } });

/** Stores a second factor. */
export const storeMfa = (key: string, id: string, m: MfaDraft) =>
  api(`/personas/${id}/mfa`, { key, method: 'PUT', body: mfaBody(m) });

/** Removes the default second factor, or the one for a site. */
export const clearMfa = (key: string, id: string, domain?: string) =>
  api(`/personas/${id}/mfa${domain ? `?domain=${encodeURIComponent(domain)}` : ''}`, { key, method: 'DELETE' });

/** Stores a portal sign-in. */
export const storeCredential = (key: string, id: string, body: CredentialDraft) =>
  api(`/personas/${id}/credentials`, { key, method: 'PUT', body });

/** Removes a portal sign-in. */
export const removeCredential = (key: string, id: string, domain: string) =>
  api(`/personas/${id}/credentials?domain=${encodeURIComponent(domain)}`, { key, method: 'DELETE' });

/** A new identity on the same kind of device. */
export const clonePersona = (key: string, id: string) =>
  api<Persona>(`/personas/${id}/clone`, { key, method: 'POST', body: {} });

/** Deletes a persona. */
export const deletePersona = (key: string, id: string) => api(`/personas/${id}`, { key, method: 'DELETE' });

/** The proxies a persona may be pinned to; the endpoint has answered both as a list and wrapped. */
export const fetchProxyChoices = (key: string) =>
  api<ProxyList>('/proxies', { key }).then((r) => (Array.isArray(r) ? r : r.proxies || []));

/** Every proxy this key can use. */
export const listProxies = (key: string) => api<ProxyTable>('/proxies', { key }).then((r) => r.proxies);

/** The body POST /proxies takes for the form: blanks left for the server to fill. */
const proxyBody = (d: ProxyDraft) => ({
  label: d.label || undefined,
  url: d.url,
  geo: d.geo || undefined,
  kind: d.kind,
  maxPersonas: Number(d.max) || 1,
});

/** Adds a proxy from the form. */
export const addProxy = (key: string, d: ProxyDraft) =>
  api<ProxyRow>('/proxies', { key, method: 'POST', body: proxyBody(d) });

/** Checks every proxy's exit. */
export const checkProxies = (key: string) =>
  api<CheckAnswer>('/proxies/check', { key, method: 'POST', body: {} }).then((r) => r.results);

/** Removes a proxy. */
export const deleteProxy = (key: string, id: string) => api(`/proxies/${id}`, { key, method: 'DELETE' });
