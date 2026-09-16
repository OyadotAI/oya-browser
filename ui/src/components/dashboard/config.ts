import { apiUrl, apiKeyHeaders, apiOrigin } from '@/lib/api';

/** What GET /api/config returns. Settings belong to the API key, not an account. */
export interface KeyConfig {
  llm_provider: string;
  openai_api_key: string;       // masked
  openai_base_url: string;
  chat_model: string;
  browser_provider: string;
  anchor_api_key: string;
  browserbase_api_key: string;
  browserbase_project_id: string;
  steel_api_key: string;
  browseruse_api_key: string;
  cdp_ws_url: string;
  captcha_solver: string;
  captcha_api_key: string;
  onboarded: string;
  desktop_seen_at: string;
  has_openai_key: boolean;
  inherited: boolean;
  effective: { baseUrl: string; model: string; hasLlmKey: boolean };
  providers: Array<{ id: string; label: string; needs: string[]; configured: boolean }>;
}

/** Prefer the server's own reason — it explains what a bare status code cannot. */
async function reason(res: Response, fallback: string): Promise<string> {
  try {
    const body = await res.json();
    if (typeof body?.error === 'string' && body.error) return body.error;
  } catch { /* not JSON */ }
  return `${fallback} (${res.status})`;
}

export async function loadConfig(apiKey: string): Promise<KeyConfig> {
  const res = await fetch(apiUrl('/config'), { headers: apiKeyHeaders(apiKey) });
  if (!res.ok) throw new Error(await reason(res, 'Could not load settings'));
  return res.json();
}

export async function saveConfig(apiKey: string, values: Record<string, string>): Promise<KeyConfig> {
  const res = await fetch(apiUrl('/config'), {
    method: 'POST',
    headers: apiKeyHeaders(apiKey),
    body: JSON.stringify(values),
  });
  if (!res.ok) throw new Error(await reason(res, 'Could not save'));
  return res.json();
}

export const LLM_PRESETS = [
  { id: 'anthropic', label: 'Claude', model: 'claude-sonnet-4-5', hint: 'sk-ant-...' },
  { id: 'openai', label: 'OpenAI', model: 'gpt-4o-mini', hint: 'sk-...' },
  { id: 'gemini', label: 'Gemini', model: 'gemini-3.8-flash', hint: 'AIza...' },
  { id: 'vertex', label: 'Gemini Enterprise', model: 'gemini-2.5-flash', hint: 'AIza... (express mode)' },
];

/** Whether a provider runs on our own infrastructure, and so can reuse desktop cookies. */
export const isOyaProvider = (id: string) => id === 'oya-cloud' || id === 'oya-selfhosted';

/**
 * One-click desktop sign-in. The installed browser registers `oya://`, so this
 * hands it a single-use pairing code and the server to exchange it with.
 *
 * The key itself never goes in the URL: a protocol link is reachable by any page
 * the user visits, and it lands in OS logs on the way. The code expires in
 * minutes, redeems once, and the desktop app still asks before acting on it.
 */
export async function desktopSignInUrl(apiKey: string, profile = 'default'): Promise<string> {
  const res = await fetch(apiUrl('/pairing'), { method: 'POST', headers: apiKeyHeaders(apiKey), body: JSON.stringify({ profile }) });
  if (!res.ok) throw new Error(await reason(res, 'Could not start desktop sign-in'));
  const { code } = await res.json();
  const ws = `${apiOrigin().replace(/^http/, 'ws')}/ws`;
  return `oya://connect?code=${encodeURIComponent(code)}&server=${encodeURIComponent(ws)}`;
}
