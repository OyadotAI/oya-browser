/**
 * The HTTP and WebSocket origins that snippets point at.
 */
import { apiOrigin } from '@/lib/api';

/** Where this dashboard is served from is where the API is. */
export function origins() {
  if (typeof window === 'undefined') return { http: '', ws: '' };
  const http = apiOrigin();
  return { http, ws: http.replace(/^http/, 'ws') };
}
