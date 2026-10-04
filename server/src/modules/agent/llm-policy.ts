/**
 * A project's model policy, applied before any page text or screenshot leaves for a
 * model provider. A project may limit which providers see its pages (a HIPAA
 * project may allow none); a key with no project, as on self-host or the CLI, keeps
 * every provider.
 */
import { audit } from '../../platform/audit.ts';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { control, projectId, llmAllowed } from '../control/service.ts';

/**
 * Who receives the page content, by the host it is sent to. The transport
 * adapter cannot answer this: Gemini, OpenRouter and any custom base URL all
 * speak the OpenAI protocol, yet send the content to someone other than OpenAI.
 */
const VENDOR_HOSTS: Array<[string, (host: string) => boolean]> = [
  ['anthropic', (host) => host === 'api.anthropic.com'],
  ['openai', (host) => host === 'api.openai.com'],
  ['gemini', (host) => host === 'googleapis.com' || host.endsWith('.googleapis.com')],
];

/** The vendor `baseUrl` sends content to; any other host is `other (<host>)`, which no allow list names. */
export function vendorOf(baseUrl: string) {
  const host = URL.parse(baseUrl)?.hostname.toLowerCase() || '';
  return VENDOR_HOSTS.find(([, owns]) => owns(host))?.[0] ?? `other (${host || 'unknown host'})`;
}

/** The vendor `baseUrl` reaches, when the key's project refuses it; null when it is allowed. */
async function refusedProvider(apiKey, baseUrl, service) {
  const project = apiKey ? await service.store.get('project', projectId(apiKey)) : null;
  const provider = vendorOf(baseUrl);
  return project && !llmAllowed(project.settings, provider) ? { project, provider } : null;
}

/** Records the refusal, so a compliance review sees every time a model was kept out. */
const auditRefusal = (apiKey, { project, provider }) =>
  audit({
    action: 'llm.refused',
    actorKey: apiKey,
    targetType: 'project',
    targetId: project.id,
    outcome: 'denied',
    meta: { provider },
  });

/** Refuses with 403 when the key's project does not let page content go to the provider at `baseUrl`. */
export async function requireLlmAllowed(apiKey, baseUrl, service = control()) {
  const refused = await refusedProvider(apiKey, baseUrl, service);
  if (!refused) return;
  auditRefusal(apiKey, refused);
  throw new HttpError(
    Status.FORBIDDEN,
    `This project's policy does not allow sending page content to ${refused.provider}. Change it in the console under Project operations > Limits and retention, or pick an allowed model.`,
    { code: 'llm_not_allowed' },
  );
}

/** The key's model settings when its project allows them, else null, for work that can go on without a model. */
export async function allowedLlm(apiKey, llm, service = control()) {
  if (!llm?.openaiKey) return llm;
  return requireLlmAllowed(apiKey, llm.baseUrl, service).then(
    () => llm,
    () => null,
  );
}
