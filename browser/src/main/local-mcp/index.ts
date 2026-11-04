/** Facade: Oya's MCP port on this computer, and the coding agents that can be connected to it. */
import type { AppServices } from '../app/services.ts';
import { LocalMcp } from './server.ts';
import { AGENTS, type Endpoint } from './agents.ts';
import { agentInstallation, agentPlatform } from './agent-platform.ts';
import type { LocalAgent } from '../../shared/ipc.ts';

export { LocalMcp } from './server.ts';
export { AGENTS, installedAgents, type AgentStatus } from './agents.ts';
export { AgentLauncher } from './launcher.ts';

/** Current-instance status must compare URL and credentials without returning either to the renderer. */
export function knownAgents(endpoint: Endpoint, all = false, platform = agentPlatform()): LocalAgent[] {
  return Object.entries(AGENTS)
    .map((entry) => agentStatus(entry, endpoint, platform))
    .filter((agent) => all || agent.installed);
}
/** Each signal is derived from a real installation and this browser's exact MCP endpoint. */
function agentStatus(
  [id, agent]: [string, (typeof AGENTS)[string]],
  endpoint: Endpoint,
  platform: ReturnType<typeof agentPlatform>,
): LocalAgent {
  const installed = !!agentInstallation(id, platform);
  const launchable = installed && ['darwin', 'win32'].includes(platform.platform);
  return { id, name: agent.name, installed, launchable, connected: installed && agent.connected(endpoint) };
}

/** Whether this browser offers the port: a person's desktop, not a governed or cloud runtime, unless turned off. */
export function offersLocalMcp(deps: Pick<AppServices, 'governance'>, platform = process.platform): boolean {
  if (process.env.OYA_LOCAL_MCP === '0' || deps.governance.configuration) return false;
  return platform === 'darwin' || platform === 'win32';
}

/** Starts the port when this browser offers it; answers it either way so the footer can say. */
export async function startLocalMcp(deps: ConstructorParameters<typeof LocalMcp>[0] & Pick<AppServices, 'governance'>) {
  const mcp = new LocalMcp(deps);
  if (offersLocalMcp(deps)) await mcp.start();
  return mcp;
}
